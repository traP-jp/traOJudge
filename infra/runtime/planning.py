"""Bounded all-success DAG lookahead; no claim to predict unknown outcomes/arrivals."""
def validate_parents(parents, size):
    if not isinstance(parents, list) or len(parents) != size:
        raise ValueError("test_parents must have one null/index entry per case")
    for i, parent in enumerate(parents):
        if parent is not None and (type(parent) is not int or not 0 <= parent < size or parent == i):
            raise ValueError("invalid test parent")
    for i, parent in enumerate(parents):
        seen = {i}
        while parent is not None:
            if parent in seen:
                raise ValueError("test parent cycle")
            seen.add(parent)
            parent = parents[parent]
    return parents


def project(jobs, edges, states=None, horizon=None):
    """Peak all-success earliest-start execution demand inside a bounded horizon.

    This models an all-success scenario, not a mathematical upper bound on actual
    demand within the horizon: current work is charged its full wall limit,
    unknown gates pass, and cancelled/failed ancestors prune work. A gate that
    finishes earlier than its wall limit can expose more demand within the horizon.
    Each execution corresponds to one VM. No arrival or warm-up model is implied.
    """
    if horizon is None:
        from policy import load_policy
        horizon = load_policy()["scaling"]["lookahead_seconds"]
    if type(horizon) is not int or horizon <= 0:
        raise ValueError("lookahead horizon must be a positive integer")
    states = states or {}
    tasks = {node: task for node, task in jobs}
    if len(tasks) != len(jobs):
        raise ValueError("duplicate graph node")
    incoming = {node: [] for node in tasks}
    outgoing = {node: [] for node in tasks}
    for left, right in edges:
        if left not in tasks or right not in tasks:
            raise ValueError("unknown graph edge endpoint")
        incoming[right].append(left)
        outgoing[left].append(right)
    # Kahn's traversal avoids recursion limits on deep fail-fast case trees.
    remaining = {node: len(parents) for node, parents in incoming.items()}
    queue = [node for node, count in remaining.items() if count == 0]
    finish = {}
    blocked = set()
    intervals = []
    for node in queue:
        status = states.get(node)
        if status in ["failed", "cancelled"] or any(parent in blocked for parent in incoming[node]):
            blocked.add(node)
            finish[node] = 0
        elif status == "completed":
            finish[node] = 0
        else:
            duration = tasks[node]["wall_seconds"]
            if not isinstance(duration, (int, float)) or isinstance(duration, bool) or duration <= 0:
                raise ValueError("node wall_seconds must be positive")
            start = max((finish[parent] for parent in incoming[node]), default=0)
            finish[node] = start + duration
            if start < horizon:
                intervals.extend([(start, 1), (min(start + duration, horizon), -1)])
        for child in outgoing[node]:
            remaining[child] -= 1
            if remaining[child] == 0:
                queue.append(child)
    if len(finish) != len(tasks):
        raise ValueError("graph cycle")
    active = peak = 0
    # End events sort before start events; sequential nodes do not overlap.
    for _, delta in sorted(intervals):
        active += delta
        peak = max(peak, active)
    ready = sum(states.get(n) in ["ready", "starting", "running"] and n not in blocked for n in tasks)
    if not states:
        ready = sum(not incoming[n] for n in tasks)
    return {"ready": ready, "potential_slots": peak, "horizon_seconds": horizon,
            "assumption": "all unresolved gates pass; full remaining wall-limit durations; one execution per VM"}
