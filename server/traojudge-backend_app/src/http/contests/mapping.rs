use crate::http::schema;
use chrono::{DateTime, Utc};
use traojudge_core::domain::model::contest::*;

fn time(field: &'static str, value: &str) -> Result<DateTime<Utc>, ContestError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.to_utc())
        .map_err(|_| ContestError::validation(field, "must be an RFC3339 timestamp"))
}
impl From<schema::ContestParticipationMode> for ParticipationMode {
    fn from(value: schema::ContestParticipationMode) -> Self {
        match value {
            schema::ContestParticipationMode::Individual => Self::Individual,
            schema::ContestParticipationMode::Team => Self::Team,
        }
    }
}
impl From<ParticipationMode> for schema::ContestParticipationMode {
    fn from(value: ParticipationMode) -> Self {
        match value {
            ParticipationMode::Individual => Self::Individual,
            ParticipationMode::Team => Self::Team,
        }
    }
}
impl From<schema::VisibilityValue> for ContestVisibility {
    fn from(value: schema::VisibilityValue) -> Self {
        match value {
            schema::VisibilityValue::Public => Self::Public,
            schema::VisibilityValue::Private => Self::Private,
        }
    }
}
impl From<ContestVisibility> for schema::VisibilityValue {
    fn from(value: ContestVisibility) -> Self {
        match value {
            ContestVisibility::Public => Self::Public,
            ContestVisibility::Private => Self::Private,
        }
    }
}
impl From<schema::ContestRules> for ContestRules {
    fn from(value: schema::ContestRules) -> Self {
        Self {
            tie_break: match value.tie_break {
                schema::contest_rules::TieBreak::Icpc => TieBreak::Icpc,
                schema::contest_rules::TieBreak::Atcoder => TieBreak::Atcoder,
            },
            penalty_minutes: value.penalty_minutes,
        }
    }
}
impl From<ContestRules> for schema::ContestRules {
    fn from(value: ContestRules) -> Self {
        Self::new(
            match value.tie_break {
                TieBreak::Icpc => schema::contest_rules::TieBreak::Icpc,
                TieBreak::Atcoder => schema::contest_rules::TieBreak::Atcoder,
            },
            value.penalty_minutes,
        )
    }
}
impl TryFrom<schema::ContestCreate> for NewContest {
    type Error = ContestError;
    fn try_from(value: schema::ContestCreate) -> Result<Self, Self::Error> {
        Ok(Self {
            title: value.title,
            description: value.description.unwrap_or_default(),
            visibility: value
                .visibility
                .unwrap_or(schema::VisibilityValue::Private)
                .into(),
            participation_mode: value.participation_mode.into(),
            starts_at: time("startsAt", &value.starts_at)?,
            ends_at: time("endsAt", &value.ends_at)?,
            rules: value.rules.into(),
        })
    }
}
impl TryFrom<schema::VirtualContestCreate> for NewContest {
    type Error = ContestError;
    fn try_from(value: schema::VirtualContestCreate) -> Result<Self, Self::Error> {
        Ok(Self {
            title: value.title,
            description: String::new(),
            visibility: value
                .visibility
                .unwrap_or(schema::VisibilityValue::Private)
                .into(),
            participation_mode: ParticipationMode::Individual,
            starts_at: time("startsAt", &value.starts_at)?,
            ends_at: time("endsAt", &value.ends_at)?,
            rules: value.rules.into(),
        })
    }
}
impl TryFrom<schema::ContestUpdate> for ContestChanges {
    type Error = ContestError;
    fn try_from(value: schema::ContestUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            title: value.title,
            visibility: value.visibility.map(Into::into),
            description: value.description,
            participation_mode: value.participation_mode.map(Into::into),
            starts_at: value.starts_at.map(|s| time("startsAt", &s)).transpose()?,
            ends_at: value.ends_at.map(|s| time("endsAt", &s)).transpose()?,
            rules: value.rules.map(Into::into),
        })
    }
}
impl TryFrom<schema::VirtualContestUpdate> for ContestChanges {
    type Error = ContestError;
    fn try_from(value: schema::VirtualContestUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            title: value.title,
            visibility: value.visibility.map(Into::into),
            starts_at: value.starts_at.map(|s| time("startsAt", &s)).transpose()?,
            ends_at: value.ends_at.map(|s| time("endsAt", &s)).transpose()?,
            rules: value.rules.map(Into::into),
            ..Self::default()
        })
    }
}
impl From<Contest> for schema::Contest {
    fn from(c: Contest) -> Self {
        Self {
            id: c.id,
            title: c.title,
            description: Some(c.description),
            visibility: c.visibility.into(),
            participation_mode: c.participation_mode.into(),
            starts_at: c.starts_at.to_rfc3339(),
            ends_at: c.ends_at.to_rfc3339(),
            rules: c.rules.into(),
            revision: c.revision,
        }
    }
}
impl From<Contest> for schema::VirtualContest {
    fn from(c: Contest) -> Self {
        Self::new(
            c.id,
            c.title,
            c.visibility.into(),
            c.starts_at.to_rfc3339(),
            c.ends_at.to_rfc3339(),
            c.rules.into(),
            c.revision,
        )
    }
}
impl From<schema::ContestProblemInput> for ContestProblemInput {
    fn from(value: schema::ContestProblemInput) -> Self {
        Self {
            label: value.label,
            order: value.order,
        }
    }
}
impl From<ContestProblem> for schema::ContestProblem {
    fn from(value: ContestProblem) -> Self {
        Self::new(value.problem_id, value.label, value.order)
    }
}
impl From<Participation> for schema::Participation {
    fn from(value: Participation) -> Self {
        Self::new(value.user_id, value.joined_at.to_rfc3339())
    }
}
impl From<TeamMember> for schema::ContestTeamMember {
    fn from(value: TeamMember) -> Self {
        Self::new(value.user_id, value.display_name)
    }
}
impl From<ContestTeam> for schema::ContestTeam {
    fn from(value: ContestTeam) -> Self {
        Self::new(
            value.id,
            value.name,
            value.members.into_iter().map(Into::into).collect(),
        )
    }
}
impl From<TeamParticipation> for schema::ContestTeamParticipation {
    fn from(value: TeamParticipation) -> Self {
        Self::new(value.team.into(), value.joined_at.to_rfc3339())
    }
}
