"""Pure, versioned pool policy. No AWS imports or implicit experimental overrides."""
import copy
import json
import os
import re
import sys


def _fields(value, expected, name):
    if not isinstance(value, dict) or set(value) != set(expected):
        raise ValueError(f"{name} must contain exactly {sorted(expected)}")


def _integer(value, low, high, name):
    if type(value) is not int or not low <= value <= high:
        raise ValueError(f"{name} must be an integer in [{low}, {high}]")


def validate_policy(policy):
    _fields(policy, ["version", "profile", "limits", "placement", "queue", "host", "scaling", "capacity"], "policy")
    if policy["version"] != 1 or type(policy["version"]) is not int:
        raise ValueError("unsupported policy version")
    if not isinstance(policy["profile"], str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,64}", policy["profile"]):
        raise ValueError("profile must be named")
    _fields(policy["limits"], ["max_instances", "executions_per_instance"], "limits")
    _integer(policy["limits"]["max_instances"], 1, 5, "max_instances")
    _integer(policy["limits"]["executions_per_instance"], 1, 1, "executions_per_instance")
    sections = {
        "placement": {"availability_zone_count": (1, 1), "replacement_min_healthy_percentage": (0, 0),
                      "replacement_max_healthy_percentage": (100, 100)},
        "queue": {"visibility_seconds": (30, 43200), "max_receive_count": (2, 1000),
                  "renewal_seconds": (1, 21600)},
        "host": {"heartbeat_seconds": (1, 60), "heartbeat_stale_seconds": (3, 300),
                 "poll_seconds": (1, 20), "container_timeout_seconds": (1, 3600)},
        "scaling": {"lookahead_seconds": (1, 3600), "idle_seconds": (0, 86400),
                    "scale_in_seconds": (0, 3600), "metric_period_seconds": (60, 60),
                    "demand_stale_seconds": (60, 86400), "lease_seconds": (60, 300)},
    }
    for section, fields in sections.items():
        _fields(policy[section], fields, section)
        for field, bounds in fields.items():
            _integer(policy[section][field], *bounds, f"{section}.{field}")
    host, queue = policy["host"], policy["queue"]
    if host["heartbeat_stale_seconds"] <= host["heartbeat_seconds"] * 2:
        raise ValueError("heartbeat expiry must allow at least two heartbeat intervals")
    if queue["renewal_seconds"] * 2 > queue["visibility_seconds"]:
        raise ValueError("renewal interval must allow at least two attempts per visibility window")
    if queue["visibility_seconds"] <= host["container_timeout_seconds"]:
        raise ValueError("visibility must exceed container timeout")
    _fields(policy["capacity"], ["strategy", "operating_limit", "warm_instances"], "capacity")
    capacity = policy["capacity"]
    if capacity["strategy"] not in ["lookahead", "reactive"]:
        raise ValueError("unknown capacity strategy")
    _integer(capacity["operating_limit"], 1, policy["limits"]["max_instances"], "operating_limit")
    _integer(capacity["warm_instances"], 0, capacity["operating_limit"], "warm_instances")
    return policy


def compile_policy(document, profile):
    _fields(document, ["version", "limits", "placement", "queue", "host", "scaling", "profiles"], "document")
    if not isinstance(document["profiles"], dict) or profile not in document["profiles"]:
        raise ValueError(f"unknown policy profile: {profile}")
    policy = copy.deepcopy({k: v for k, v in document.items() if k != "profiles"})
    policy.update(profile=profile, capacity=copy.deepcopy(document["profiles"][profile]))
    return validate_policy(policy)


def load_policy():
    """Deployment must inject the exact compiled document, including its profile."""
    return validate_policy(json.loads(os.environ["POOL_POLICY"]))


def target_instances(policy, ready, forecast, pending_control, now, activity, sleeping):
    """One execution requires one VM; forecast is a modeled all-gates-pass scenario."""
    capacity = policy["capacity"]
    demand = max(ready, forecast if capacity["strategy"] == "lookahead" else 0)
    retain = not sleeping or now - activity < policy["scaling"]["idle_seconds"]
    floor = capacity["warm_instances"] if retain else 0
    if pending_control:
        floor = max(1, floor)
    return min(capacity["operating_limit"], max(demand, floor))


def retirement_plan(target, desired, instances, hosts, retiring, drain_ids):
    """Irreversible drain is the fence against delayed CloudWatch scale-in actions.

    Complete existing retirements before publishing a rebound. Unknown/pending or
    quarantined hosts cannot acknowledge drain; keep them protected and hold.
    """
    current = max(desired, len(instances))
    draining = [i for i in retiring if i in instances]
    needed = max(0, current - target)
    candidates = sorted((i for i, host in hosts.items()
                         if i not in retiring and host["state"] == "ready"),
                        key=lambda i: (hosts[i]["active"], i))
    selected = candidates[:max(0, needed - len(draining))]
    retiring_now = draining + selected
    acknowledged = [i for i in draining if i in hosts
                    and hosts[i]["state"] == "drained" and hosts[i].get("drain_id") == drain_ids.get(i)
                    and hosts[i].get("active") == 0 and hosts[i].get("polling") is False
                    and not instances[i]["ProtectedFromScaleIn"]]
    if retiring_now:
        safe = current - len(acknowledged)
        return safe, selected, "request_scale_in_after_drain" if acknowledged else "await_drain"
    if target < current:
        # No eligible host means no drain acknowledgment can justify reduction.
        # ASG protection is an independent fence, not permission to lower desired.
        return current, [], "await_unknown_capacity"
    return target, [], "request_scale_out" if target > desired else "hold"


if __name__ == "__main__":
    with open(sys.argv[1], encoding="utf-8") as source:
        print(json.dumps(compile_policy(json.load(source), sys.argv[2]), sort_keys=True, separators=(",", ":")))
