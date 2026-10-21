use std::collections::{HashMap, HashSet, VecDeque};

use crate::payload::Payload;
use crate::storage::{
    CompareAndSetJobStateError, CompareAndSetPipelineStateError, CreatePipelineError,
    EdgeRepository, JobRepository, PipelineRepository, UnitOfWork, UnitOfWorkProvider,
};
use crate::{JobId, JobStartCallback, JobState, PipelineId, PipelineState};

mod error;

pub use error::{
    CancelPipelineError, DispatchJobError, JobCompletedError, JobFailedError, StartPipelineError,
};

const ALL_STATES: [JobState; 7] = [
    JobState::Waiting,
    JobState::Ready,
    JobState::Starting,
    JobState::Running,
    JobState::Completed,
    JobState::Failed,
    JobState::Cancelled,
];

pub struct Engine<Provider, Callback> {
    provider: Provider,
    callback: Callback,
}

impl<Provider: UnitOfWorkProvider, Callback: JobStartCallback> Engine<Provider, Callback> {
    pub fn new(provider: Provider, callback: Callback) -> Self {
        Self { provider, callback }
    }

    pub async fn start_pipeline(
        &self,
        pipeline_id: PipelineId,
        jobs: Vec<(JobId, Payload<Callback::Input>)>,
        edges: &[(JobId, JobId)],
    ) -> Result<(), StartPipelineError<Provider::Error, Callback::Error>> {
        let job_ids: Vec<_> = jobs.iter().map(|(job_id, _)| *job_id).collect();
        let roots = validate_pipeline(&job_ids, edges)?;
        let jobs = jobs
            .into_iter()
            .map(|(job_id, payload)| {
                serde_json::to_value(payload)
                    .map(|payload| (job_id, payload))
                    .map_err(StartPipelineError::Payload)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut work = self
            .provider
            .begin()
            .await
            .map_err(StartPipelineError::Storage)?;
        let created = work.pipelines().create(pipeline_id).await;
        match created {
            Ok(()) => {
                let requests: Vec<_> = jobs
                    .into_iter()
                    .map(|(job_id, payload)| (job_id, pipeline_id, payload))
                    .collect();
                for result in work.jobs().create_many(&requests).await {
                    result?;
                }
                work.edges().register(edges).await?;
                let updates: Vec<_> = roots
                    .into_iter()
                    .map(|job_id| (job_id, JobState::Waiting, JobState::Ready))
                    .collect();
                for result in work.jobs().compare_and_set_state_many(&updates).await {
                    result?;
                }
                if job_ids.is_empty() {
                    work.pipelines()
                        .compare_and_set_state(
                            pipeline_id,
                            PipelineState::Running,
                            PipelineState::Completed,
                        )
                        .await?;
                }
            }
            Err(CreatePipelineError::AlreadyExists(_)) => {
                if !lock_pipeline(&mut work, pipeline_id).await? {
                    work.commit().await.map_err(StartPipelineError::Storage)?;
                    return Ok(());
                }
                if work.jobs().count(pipeline_id, &ALL_STATES).await? != job_ids.len() as u64 {
                    return Err(StartPipelineError::DefinitionContradicts(pipeline_id));
                }
                for (job_id, payload) in jobs {
                    if work.jobs().get_pipeline_id(job_id).await? != pipeline_id {
                        return Err(StartPipelineError::DefinitionContradicts(pipeline_id));
                    }
                    let stored = work.jobs().get_payload(job_id).await?;
                    let stored: Payload<Callback::Input> =
                        serde_json::from_value(stored).map_err(StartPipelineError::Payload)?;
                    let stored =
                        serde_json::to_value(stored).map_err(StartPipelineError::Payload)?;
                    if stored != payload {
                        return Err(StartPipelineError::DefinitionContradicts(pipeline_id));
                    }
                    let expected: HashSet<_> =
                        if work.jobs().get(job_id).await? == JobState::Completed {
                            HashSet::new()
                        } else {
                            edges
                                .iter()
                                .filter_map(|&(from, to)| (from == job_id).then_some(to))
                                .collect()
                        };
                    let actual: HashSet<_> =
                        work.edges().list_next(job_id).await?.into_iter().collect();
                    if actual != expected {
                        return Err(StartPipelineError::DefinitionContradicts(pipeline_id));
                    }
                }
            }
            Err(error) => return Err(error.into()),
        }
        work.commit().await.map_err(StartPipelineError::Storage)?;
        self.dispatch_pipeline(pipeline_id).await?;
        Ok(())
    }

    pub async fn cancel_pipeline(
        &self,
        pipeline_id: PipelineId,
    ) -> Result<(), CancelPipelineError<Provider::Error>> {
        let mut work = self
            .provider
            .begin()
            .await
            .map_err(CancelPipelineError::Storage)?;
        match work
            .pipelines()
            .compare_and_set_state(
                pipeline_id,
                PipelineState::Running,
                PipelineState::Cancelled,
            )
            .await
        {
            Ok(()) => {}
            Err(CompareAndSetPipelineStateError::StateContradicts {
                actual: PipelineState::Completed | PipelineState::Cancelled,
                ..
            }) => {}
            Err(error) => return Err(error.into()),
        }
        work.commit().await.map_err(CancelPipelineError::Storage)
    }

    pub async fn job_failed(&self, job_id: JobId) -> Result<(), JobFailedError<Provider::Error>> {
        let mut work = self
            .provider
            .begin()
            .await
            .map_err(JobFailedError::Storage)?;
        let pipeline_id = work.jobs().get_pipeline_id(job_id).await?;
        if lock_pipeline(&mut work, pipeline_id).await?
            && finish_job(&mut work, job_id, JobState::Failed).await?
        {
            work.pipelines()
                .compare_and_set_state(
                    pipeline_id,
                    PipelineState::Running,
                    PipelineState::Cancelled,
                )
                .await?;
        }
        work.commit().await.map_err(JobFailedError::Storage)
    }

    pub async fn job_completed(
        &self,
        job_id: JobId,
    ) -> Result<(), JobCompletedError<Provider::Error, Callback::Error>> {
        let mut work = self
            .provider
            .begin()
            .await
            .map_err(JobCompletedError::Storage)?;
        let pipeline_id = work.jobs().get_pipeline_id(job_id).await?;
        if !lock_pipeline(&mut work, pipeline_id).await? {
            work.commit().await.map_err(JobCompletedError::Storage)?;
            return Ok(());
        }
        if finish_job(&mut work, job_id, JobState::Completed).await? {
            let next_jobs = work.edges().remove_from(job_id).await?;
            for next in next_jobs {
                if work.edges().count_incoming(next).await? == 0
                    && work.jobs().get(next).await? == JobState::Waiting
                {
                    work.jobs()
                        .compare_and_set_state(next, JobState::Waiting, JobState::Ready)
                        .await?;
                }
            }
        }
        let remaining: Vec<_> = ALL_STATES
            .into_iter()
            .filter(|&state| state != JobState::Completed)
            .collect();
        if work.jobs().count(pipeline_id, &remaining).await? == 0 {
            work.pipelines()
                .compare_and_set_state(
                    pipeline_id,
                    PipelineState::Running,
                    PipelineState::Completed,
                )
                .await?;
        }
        work.commit().await.map_err(JobCompletedError::Storage)?;
        self.dispatch_pipeline(pipeline_id).await?;
        Ok(())
    }

    async fn dispatch_pipeline(
        &self,
        pipeline_id: PipelineId,
    ) -> Result<(), DispatchJobError<Provider::Error, Callback::Error>> {
        let mut work = self
            .provider
            .begin()
            .await
            .map_err(DispatchJobError::Storage)?;
        let pending = work
            .jobs()
            .list_by_state(pipeline_id, &[JobState::Ready, JobState::Starting])
            .await?;
        work.commit().await.map_err(DispatchJobError::Storage)?;
        self.dispatch_jobs(&pending).await
    }

    async fn dispatch_jobs(
        &self,
        job_ids: &[JobId],
    ) -> Result<(), DispatchJobError<Provider::Error, Callback::Error>> {
        let mut first_error = None;
        for &job_id in job_ids {
            if let Err(error) = self.dispatch_job(job_id).await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    async fn dispatch_job(
        &self,
        job_id: JobId,
    ) -> Result<(), DispatchJobError<Provider::Error, Callback::Error>> {
        let mut work = self
            .provider
            .begin()
            .await
            .map_err(DispatchJobError::Storage)?;
        let pipeline_id = work.jobs().get_pipeline_id(job_id).await?;
        if !lock_pipeline(&mut work, pipeline_id).await? {
            work.commit().await.map_err(DispatchJobError::Storage)?;
            return Ok(());
        }
        let prepared = work
            .jobs()
            .compare_and_set_state(job_id, JobState::Ready, JobState::Starting)
            .await;
        match prepared {
            Ok(()) => {}
            Err(CompareAndSetJobStateError::StateContradicts {
                actual: JobState::Starting,
                ..
            }) => {}
            Err(CompareAndSetJobStateError::StateContradicts { .. }) => {
                work.commit().await.map_err(DispatchJobError::Storage)?;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        }
        let stored = work.jobs().get_payload(job_id).await?;
        let payload = serde_json::from_value(stored)
            .map_err(|source| DispatchJobError::Payload { job_id, source })?;
        work.commit().await.map_err(DispatchJobError::Storage)?;

        self.callback
            .start(job_id, payload)
            .await
            .map_err(|source| DispatchJobError::Callback { job_id, source })?;

        let mut work = self
            .provider
            .begin()
            .await
            .map_err(DispatchJobError::Storage)?;
        if lock_pipeline(&mut work, pipeline_id).await? {
            match work
                .jobs()
                .compare_and_set_state(job_id, JobState::Starting, JobState::Running)
                .await
            {
                Ok(()) => {}
                Err(CompareAndSetJobStateError::StateContradicts {
                    actual:
                        JobState::Running | JobState::Completed | JobState::Failed | JobState::Cancelled,
                    ..
                }) => {}
                Err(error) => return Err(error.into()),
            }
        }
        work.commit().await.map_err(DispatchJobError::Storage)
    }
}

async fn lock_pipeline<Work: UnitOfWork>(
    work: &mut Work,
    pipeline_id: PipelineId,
) -> Result<bool, CompareAndSetPipelineStateError<Work::Error>> {
    match work
        .pipelines()
        .compare_and_set_state(pipeline_id, PipelineState::Running, PipelineState::Running)
        .await
    {
        Ok(()) => Ok(true),
        Err(CompareAndSetPipelineStateError::StateContradicts {
            actual: PipelineState::Completed | PipelineState::Cancelled,
            ..
        }) => Ok(false),
        Err(error) => Err(error),
    }
}

async fn finish_job<Work: UnitOfWork>(
    work: &mut Work,
    job_id: JobId,
    next: JobState,
) -> Result<bool, CompareAndSetJobStateError<Work::Error>> {
    let mut expected = JobState::Running;
    loop {
        match work
            .jobs()
            .compare_and_set_state(job_id, expected, next)
            .await
        {
            Ok(()) => return Ok(true),
            Err(CompareAndSetJobStateError::StateContradicts {
                actual: actual @ (JobState::Starting | JobState::Running),
                ..
            }) => expected = actual,
            Err(CompareAndSetJobStateError::StateContradicts {
                actual: JobState::Completed | JobState::Failed | JobState::Cancelled,
                ..
            }) => return Ok(false),
            Err(error) => return Err(error),
        }
    }
}

fn validate_pipeline<
    StorageError: std::error::Error + 'static,
    CallbackError: std::error::Error + 'static,
>(
    job_ids: &[JobId],
    edges: &[(JobId, JobId)],
) -> Result<Vec<JobId>, StartPipelineError<StorageError, CallbackError>> {
    let mut incoming = HashMap::new();
    let mut outgoing: HashMap<JobId, Vec<JobId>> = HashMap::new();
    for &job_id in job_ids {
        if incoming.insert(job_id, 0_usize).is_some() {
            return Err(StartPipelineError::DuplicateJob(job_id));
        }
    }
    let mut seen = HashSet::new();
    for &(from, to) in edges {
        if !incoming.contains_key(&from) {
            return Err(StartPipelineError::UnknownJob(from));
        }
        let count = incoming
            .get_mut(&to)
            .ok_or(StartPipelineError::UnknownJob(to))?;
        if !seen.insert((from, to)) {
            return Err(StartPipelineError::DuplicateEdge { from, to });
        }
        *count += 1;
        outgoing.entry(from).or_default().push(to);
    }
    let roots: Vec<_> = job_ids
        .iter()
        .copied()
        .filter(|job_id| incoming[job_id] == 0)
        .collect();
    let mut ready: VecDeque<_> = roots.iter().copied().collect();
    let mut visited = 0;
    while let Some(job_id) = ready.pop_front() {
        visited += 1;
        for next in outgoing.get(&job_id).into_iter().flatten() {
            let count = incoming.get_mut(next).unwrap();
            *count -= 1;
            if *count == 0 {
                ready.push_back(*next);
            }
        }
    }
    if visited != job_ids.len() {
        return Err(StartPipelineError::Cycle);
    }
    Ok(roots)
}
