"""Executable acceptance examples; upload(name, bytes) returns a pinned ObjectRef."""
LIMITS = {"wall_seconds": 30, "memory_mb": 256, "cpus": 1.0, "pids": 128, "output_bytes": 8 * 1024 * 1024}

class Example:
    def __init__(self, image, upload):
        self.image, self.upload = image, upload
        self.artifacts, self.executions = [], []
    def file(self, name, content, submission=False):
        self.artifacts.append({"name": name, "reference": self.upload(name, content.encode()), "submission": submission})
        return name
    def output(self, name):
        self.artifacts.append({"name": name, "reference": None, "submission": False})
        return name
    def node(self, name, script, inputs=(), outputs=(), case=None, peers=(), main_inputs=None, judge_errors=False, limits=None):
        script_name = self.file(name + "_init", script)
        all_inputs = [{"artifact": script_name, "environment_variable": "TRAOJUDGE_INIT_SCRIPT"}] + [{"artifact": a, "environment_variable": v} for a, v in inputs]
        self.executions.append({"name": name, "inputs": all_inputs, "outputs": list(outputs), "limits": limits or LIMITS.copy(), "peers": list(peers), "main_inputs": [script_name, *main_inputs] if main_inputs is not None else None, "case_id": case, "judge_errors": judge_errors})
    def finish(self):
        return {"schema_version": 1, "image": self.image, "artifacts": self.artifacts, "executions": self.executions}

def batch(image, upload):
    e = Example(image, upload)
    source = e.file("source", '#include <stdio.h>\nint main(){int a,b;scanf("%d%d",&a,&b);printf("%d\\n",a+b);}', True)
    data = e.file("case_input", "20 22\n")
    binary, actual = e.output("binary"), e.output("actual")
    e.node("compile", 'set -eu\ncc -x c "$TRAOJUDGE_SUBMISSION_SOURCE" -o outputs/binary\nprintf "{}\\n"\n', [(source, "TRAOJUDGE_SUBMISSION_SOURCE")], [binary])
    e.node("run", 'set -eu\n"$BINARY" < "$CASE_INPUT" > outputs/actual\nprintf "{}\\n"\n', [(binary, "BINARY"), (data, "CASE_INPUT")], [actual])
    e.node("check", 'python3 - <<\'PY\'\nimport os,json\nassert open(os.environ["ACTUAL"]).read().strip()=="42"\nprint(json.dumps({"verdict":"accepted","score":100}))\nPY\n', [(actual, "ACTUAL")], case="addition")
    return e.finish()

def heuristic(image, upload):
    e = Example(image, upload)
    source = e.file("source", 'import sys,random\nr=random.Random(int(sys.stdin.read()))\nprint(r.randrange(100))\n', True)
    scores = []
    for seed in range(2):
        data = e.file(f"seed_{seed}", str(seed))
        score = e.output(f"score_{seed}")
        script = f'''set -eu
python3 "$SOURCE" < "$SEED" > /tmp/answer
python3 - <<'PY'
import json
result={{"score":int(open("/tmp/answer").read())}}
with open("outputs/score_{seed}","w") as f: json.dump(result,f)
print(json.dumps(result))
PY
'''
        e.node(f"trial_{seed}", script, [(source,"SOURCE"),(data,"SEED")], [score], case=f"seed_{seed}")
        scores.append((score, f"SCORE_{seed}"))
    e.node("aggregate", '''python3 - <<'PY'
import os,json
scores=[json.load(open(os.environ[f"SCORE_{i}"]))["score"] for i in range(2)]
print(json.dumps({"score":sum(scores)/len(scores),"trials":len(scores)}))
PY
''', scores)
    return e.finish()

def run_twice(image, upload):
    e = Example(image, upload)
    source = e.file("source", 'import sys\nv=int(open(sys.argv[1]).read())\nopen(sys.argv[2],"w").write(str(v+1))\n', True)
    initial = e.file("initial", "0")
    first, second = e.output("state_1"), e.output("state_2")
    for index, before, after in [(1,initial,first),(2,first,second)]:
        e.node(f"run_{index}", f'set -eu\npython3 "$SOURCE" "$STATE" outputs/{after}\nprintf "{{}}\\n"\n', [(source,"SOURCE"),(before,"STATE")],[after])
    e.node("check", '''python3 - <<'PY'
import json,os
assert open(os.environ["STATE"]).read()=="2"
print(json.dumps({"verdict":"accepted","passes":2}))
PY
''', [(second,"STATE")],case="two_passes")
    return e.finish()

def interactive(image, upload):
    e = Example(image, upload)
    source = e.file("source", 'v=int(input())\nprint(v*2,flush=True)\n',True)
    peer_script = e.file("solution_init", 'exec python3 "$SOURCE" < "$TRAOJUDGE_CHANNEL_IN" > "$TRAOJUDGE_CHANNEL_OUT"\n')
    driver = e.file("interactor", '''import os,json
from pathlib import Path
c=Path(os.environ["TRAOJUDGE_CHANNEL_DIR"])/"solution"
with (c/"from-main").open("w") as send, (c/"to-main").open() as receive:
 send.write("21\\n");send.flush()
 answer=receive.readline().strip()
assert answer=="42"
Path("outputs/transcript").write_text("judge:21\\nsolution:"+answer+"\\n")
print(json.dumps({"verdict":"accepted","answer":answer}))
''')
    transcript = e.output("transcript")
    e.node("dialogue", 'exec python3 "$INTERACTOR"\n', [(driver,"INTERACTOR"),(source,"SOURCE"),(peer_script,None)], [transcript], case="dialogue", peers=[{"name":"solution","script":peer_script,"inputs":[peer_script,source],"limits":LIMITS.copy()}], main_inputs=[driver])
    return e.finish()

def coding_game(image, upload):
    e = Example(image, upload)
    inputs, peers = [], []
    for role, addition in [("a",1),("b",2)]:
        source = e.file(f"bot_{role}", f'v=int(input())\nprint(v+{addition},flush=True)\n',True)
        script = e.file(f"bot_{role}_init", f'exec python3 "$BOT_{role.upper()}" < "$TRAOJUDGE_CHANNEL_IN" > "$TRAOJUDGE_CHANNEL_OUT"\n')
        inputs += [(source,f"BOT_{role.upper()}"),(script,None)]
        peers.append({"name":role,"script":script,"inputs":[script,source],"limits":LIMITS.copy()})
    driver = e.file("referee", '''import os,json
from pathlib import Path
scores={}
for role in ["a","b"]:
 c=Path(os.environ["TRAOJUDGE_CHANNEL_DIR"])/role
 with (c/"from-main").open("w") as send,(c/"to-main").open() as receive:
  send.write("3\\n");send.flush();scores[role]=int(receive.readline())
result={"winner":max(scores,key=scores.get),"scores":scores}
Path("outputs/replay").write_text(json.dumps(result))
print(json.dumps(result))
''')
    replay = e.output("replay")
    e.node("match", 'exec python3 "$REFEREE"\n', [(driver,"REFEREE"),*inputs], [replay],case="match",peers=peers,main_inputs=[driver])
    return e.finish()

def timeout(image, upload):
    e = Example(image, upload)
    e.node("timeout", 'sleep 20\nprintf "{}\\n"\n', case="timeout",judge_errors=True,limits={**LIMITS,"wall_seconds":1})
    return e.finish()

SCENARIOS = {"batch":batch,"heuristic":heuristic,"run_twice":run_twice,"interactive":interactive,"coding_game":coding_game,"timeout":timeout}

def mixed_cases(image, upload):
    e = Example(image, upload)
    e.node("wrong_answer", 'printf \'{"verdict":"wrong_answer"}\\n\'\n',case="wa")
    e.node("time_limit", 'sleep 20\nprintf "{}\\n"\n',case="tle",judge_errors=True,limits={**LIMITS,"wall_seconds":1})
    e.node("accepted", 'printf \'{"verdict":"accepted"}\\n\'\n',case="ac")
    return e.finish()

SCENARIOS["mixed_cases"] = mixed_cases
