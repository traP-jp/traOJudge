#!/usr/bin/env python3
"""Build the graphical source with caru-ini/stack-diagram assets; no live AWS identifiers."""
import argparse
import html
import json
from pathlib import Path
import re
import subprocess
import tempfile

parser=argparse.ArgumentParser()
parser.add_argument('--skill',type=Path,required=True)
parser.add_argument('--icons',type=Path,required=True)
parser.add_argument('--draft',type=Path,required=True)
parser.add_argument('--proof',type=Path)
parser.add_argument('--output',type=Path,default=Path(__file__).with_name('aws-judge.html'))
a=parser.parse_args()
template=(a.skill/'assets/template.html').read_text()
css=re.search(r'<style>(.*?)</style>',template,re.S).group(1)
css+='''
:root{--color-data:oklch(58% .16 55);--color-active:var(--color-accent);}
[data-theme=dark]{--color-data:oklch(78% .12 55);}
body{padding:20px}.page{width:1440px;max-width:100%}.canvas,svg.edges{width:1440px;height:1080px}
.toolbar{display:flex;flex-wrap:wrap;gap:8px;margin:16px 0}button,select{font:inherit;color:var(--color-ink);background:var(--color-paper-2);border:1px solid var(--color-rule-2);border-radius:6px;padding:8px 12px;cursor:pointer}button[aria-selected=true]{border-color:var(--color-accent);color:var(--color-accent)}.panel[hidden]{display:none}.scenario{position:absolute;inset:0}.scenario[hidden]{display:none}
.node{width:176px;border:0;background:none;padding:0;border-radius:0;line-height:1.3}.node .name{font-size:14px}.node .role{font-size:11px}.node.active .tile{outline:3px solid var(--color-active);outline-offset:4px}.node:focus-visible .tile{outline:3px solid var(--color-active)}.panel-title{position:absolute;top:18px;left:28px;font-size:20px;font-weight:700}.annotation{position:absolute;color:var(--color-neutral);font-size:14px;line-height:1.8}.annotation strong{color:var(--color-ink)}
.caption{position:absolute;bottom:20px;left:28px;right:28px;font-size:14px;line-height:1.8;color:var(--color-neutral)}.data{stroke:var(--color-data);stroke-width:2;fill:none}.channel{stroke:var(--color-neutral);stroke-width:2;fill:none;stroke-dasharray:6 4}.el.data-label{fill:var(--color-data)}.e{stroke-width:2}.el{font-size:12px}.group>.g-label{font-size:13px}.panel-controls{display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin:12px 0}.panel-controls input{width:260px;accent-color:var(--color-accent)}.detail{border:1px solid var(--color-rule-2);border-radius:8px;padding:16px;margin-top:16px;font-size:14px;line-height:1.8;min-height:110px}.detail strong{display:block}.detail a{color:var(--color-accent)}.legend[hidden]{display:none}.legend{font-family:var(--font-body);font-size:13px}.legend .data-sw{border-color:var(--color-data)}.legend .channel-sw{border-color:var(--color-neutral);border-top-style:dashed}.foot{margin-top:20px}.trace{border-collapse:collapse;width:100%;font:13px var(--font-body)}.trace th,.trace td{padding:9px;border-bottom:1px solid var(--color-rule-2);text-align:left;vertical-align:top}.trace code{font:12px var(--font-mono)}.proof-wrap{max-height:630px;overflow:auto;border:1px solid var(--color-rule-2);padding:16px;margin-top:16px}.status{color:var(--color-accent);font-weight:600}.small{font-size:13px;color:var(--color-muted);line-height:1.8}.sequence-note{margin-left:auto;color:var(--color-accent);font-size:14px}
'''
base='https://github.com/traP-jp/traOJudge/blob/codex/aws-judge-execution/'
details={}
def node(id,x,y,name,role,icon,description,source='docs/services/aws-judge.md',aws=False):
    details[id]={'name':name,'description':description,'source':base+source}
    return f'<button class="node" data-id="{id}" style="left:{x-88}px;top:{y}px" aria-label="{html.escape(name)}の説明"><div class="tile'+(' bare' if aws else '')+f'"><img src="%%ICON_{icon}%%" alt=""></div><div class="name"><span>{html.escape(name)}</span></div><div class="role"><span>{html.escape(role)}</span></div></button>'
def group(x,y,w,h,label,cl='gb',icon=None):
    picture=f'<img src="%%ICON_{icon}%%" alt="">' if icon else ''
    if icon=='aws':picture='<img class="lt" src="%%ICON_aws%%" alt=""><img class="dk" src="%%ICON_aws_dark%%" alt="">'
    return f'<div class="group {cl}" style="left:{x}px;top:{y}px;width:{w}px;height:{h}px"><div class="g-label">{picture}{label}</div></div>'
def edge(path,label='',x=0,y=0,kind='e',aws=False):
    if label and x==0 and y==0:
        match=re.fullmatch(r'M([0-9]+) ([0-9]+) H([0-9]+)',path)
        if match:
            x=(int(match[1])+int(match[3]))/2
            y=int(match[2])-10
        else: raise ValueError('edge label requires explicit coordinates: '+path)
    marker='open' if aws else 'ar' 
    return f'<path class="{kind}" d="{path}" marker-end="url(#{marker})"/>'+ (f'<text class="el'+(' data-label' if kind=='data' else '')+f'" x="{x}" y="{y}" text-anchor="middle">{html.escape(label)}</text>' if label else '')
def annotation(x,y,text,width=550):
    return f'<div class="annotation" style="left:{x}px;top:{y}px;width:{width}px">{text}</div>'
def panel(id,title,groups,nodes,edges,caption,notes=(),controls=''):
    markup=f'<section class="panel" id="{id}"'+(' hidden' if id!='aws' else '')+'>'+controls+f'<div class="scroller"><div class="canvas"><div class="panel-title">{title}</div>'+''.join(groups)+'''<svg class="edges" viewBox="0 0 1440 1080" aria-hidden="true"><defs><marker id="ar" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 z" fill="context-stroke"/></marker><marker id="open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M1 1 L9 5 L1 9" fill="none" stroke="context-stroke" stroke-width="1.5"/></marker></defs>'''+''.join(edges)+'</svg>'+''.join(nodes)+''.join(notes)+f'<div class="caption">{caption}</div></div></div></section>'
    return markup.replace('id="ar"',f'id="ar_{id}"').replace('url(#ar)',f'url(#ar_{id})').replace('id="open"',f'id="open_{id}"').replace('url(#open)',f'url(#open_{id})')
panels=[]
# Placement is deliberately separate from runtime traffic (panel 03).
ns=[
node('alb',400,265,'Elastic Load Balancing','internal ALB / HTTP','alb','内部APIをControllerへ転送。インターネットには公開しない。','infra/bin/app.ts',True),
node('controller',650,265,'Amazon EC2','Controller / Rust Engine','ec2','CoreでArtifact / ExecutionのDAGからJob依存を導き、PostgreSQLへ保存。SQSで1ノードずつ依頼し、結果を受理して後続を解放する。','server/traojudge-judge/src/main.rs',True),
node('worker',875,265,'Amazon EC2','Worker / trusted supervisor','ec2','S3の版固定入力を取得・検証し、sandboxを監督。生成物のS3保存とSQS結果送信を担当。DAG解決とDB接続は行わない。','deploy/judge/worker.py',True),
node('rds',650,535,'Amazon RDS','PostgreSQL / workflow','rds','Job・依存・Pipeline、実行定義、結果inbox、成果物参照、監査イベントを永続化。ControllerのSecurity Groupだけが接続可能。','server/traojudge-infra/src/postgres/workflow/mod.rs',True),
node('requests',1200,210,'Amazon SQS','Task refs + request DLQ','sqs','1つのExecution分のTask。JobId、run_id、確定した入力ObjectRef、image、制限を含む。ファイル本体は含まない。','server/traojudge-judge/src/protocol.rs',True),
node('s3',1200,395,'Amazon S3','versioned files / logs','s3','非公開・暗号化・Versioning。入力、試行別出力・ログ、Jobごとの確定結果を保存。Workerが内容を転送する。','deploy/judge/worker.py',True),
node('results',1200,580,'Amazon SQS','NodeResult refs + result DLQ','sqs','結果JSON・終了情報・生成物ObjectRef。Controllerが永続的に受理・適用した後に削除する。','server/traojudge-judge/src/main.rs',True),
node('ssm',370,810,'AWS Systems Manager','IAM-authorized operations','ssm','管理と内部APIへの検証操作はIAMで認可したSSM経由。SSH inboundは開けない。','infra/scripts/demo.py',True),
node('iam',575,810,'AWS IAM','separate controller / worker roles','iam','ControllerとWorkerのroleを分離。WorkerはDB secretを取得不可。子sandboxに資格情報とDocker socketを渡さない。','infra/bin/app.ts',True),
node('ecr',780,810,'Amazon ECR','digest-pinned runtime','ecr','CDKで配布したimageの同じ不変digestをControllerとWorkerに設定する。','deploy/judge/Dockerfile',True),
node('logs',985,810,'Amazon CloudWatch','supervisor logs','logs','Controller / Workerと削除補助Lambdaのログ。提出プロセスのstdout / stderrはS3へ保存。','infra/bin/app.ts',True),
node('secrets',1200,810,'AWS Secrets Manager','Controller-only DB secret','secrets','DB認証情報はController roleだけが取得する。実アカウント情報はこの図に含めない。','infra/bin/app.ts',True)]
es=[edge('M436 295 H614','HTTP',525,283,'aws',True),edge('M650 390 V529','TLS / transaction',735,455,'aws',True)]
panels.append(panel('aws','01  AWSの配置と権限境界',[
    group(230,70,1170,885,'AWS Cloud / ap-northeast-1','aws','aws'),
    group(260,130,760,580,'VPC / 2 AZ','aws vpc','vpc'),
    group(290,200,700,235,'public subnets / ALB + EC2','aws public'),
    group(290,490,700,190,'isolated subnets / database','aws private')],ns,es,
    '<strong>配置はこの図、要求・結果・ファイルの通信順は「3 制御とデータ」。</strong><br>EC2のpublic subnet配置はAWS API / ECRへのoutbound用。Workerのinboundなし。NAT Gatewayなし。内部ALBは外部公開しない。',[
    annotation(25,235,'<strong>管理操作</strong><br>IAMで認可した<br>SSM経由の内部API',180),
    annotation(305,730,'<strong>運用・配布・権限</strong> — runtimeの通信経路は次の論理ビューに分ける。',1000)]))
# Logical graph, including the scripts. File nodes never become jobs.
ns=[]
for id,x,y,name,role,icon,description in [
('source',130,310,'source','Input Artifact','file','提出ソース。実行前にS3の版・SHA-256を固定する。'),('compile',370,310,'compile','Execution definition','exec','compile_initを実行しbinaryを生成。JobになるのはこのExecution。'),('binary',610,310,'binary','Intermediate Artifact','file','compileが唯一の生成元。S3へ保存して受理した参照をrunへ渡す。'),('run',850,310,'run','Execution definition','exec','binaryとcase_inputを受け取りactualを生成する。'),('actual',1090,310,'actual','Intermediate Artifact','file','runが生成したファイル。stdoutの結果JSONとは別。'),('check',1330,310,'check','Execution definition','exec','actualを読み、任意の判定JSONを返す。例はaccepted / score=100。'),('compile_init',370,120,'compile_init','Input Artifact / script','file','TRAOJUDGE_INIT_SCRIPTに結び付ける通常の入力ファイル。'),('run_init',850,120,'run_init','Input Artifact / script','file','起動スクリプト自体もファイル依存に含む。'),('check_init',1330,120,'check_init','Input Artifact / script','file','各Executionに起動スクリプトがちょうど1つ必要。'),('case',850,510,'case_input','Input Artifact','file','入力20 22。最新keyを上書きしても、このrunは指定VersionIdの内容を使う。')]:ns.append(node('dag_'+id,x,y,name,role,icon,description,'deploy/judge/examples.py'))
es=[]
for left,right in [(130,370),(370,610),(610,850),(850,1090),(1090,1330)]:es.append(edge(f'M{left+36} 340 H{right-36}',kind='data'))
for x in [370,850,1330]:es.append(edge(f'M{x} 226 V304','init script',x+60,270,'data'))
es.append(edge('M850 504 V420','input file',790,465,'data'))
ns += [node('job_compile',370,740,'compile Job','file nodes omitted','engine','Coreがファイルの生成・利用関係から導く実行依存。'),node('job_run',850,740,'run Job','waits for binary','engine','compileの結果とbinary参照の永続的な受理後に起動。'),node('job_check',1330,740,'check Job','waits for actual','engine','runの結果とactual参照の永続的な受理後に起動。')]
es += [edge('M406 770 H814','dependency',610,760),edge('M886 770 H1294','dependency',1090,760)]
panels.append(panel('dag','02  二部DAG：ファイルと実行が交互に並ぶ',[],ns,es,'<strong>上段はデータの依存。下段はそこから導いたJobの起動順。</strong><br>ArtifactにはJobを作らない。中間ファイルは生成元が1つ。起動スクリプトも通常のArtifactであり、問題定義全体とは区別する。',[annotation(35,650,'<strong>ファイルの依存を実行の依存へ縮約する</strong><br>compile → binary → run は、Jobの compile → run になる。',1000),annotation(80,900,'<strong>Artifact</strong> = 名前付きファイル　　<strong>Execution</strong> = 入出力・起動処理の定義　　<strong>Job</strong> = Engineが保存・状態管理する実行単位',1280)]))
# Read each lane left-to-right (requests/data) or right-to-left (results).
# Repeating the same participant keeps every runtime edge straight.
ns=[
node('flow_controller',180,160,'Controller','plan / schedule','engine','定義とReady / StartingをDBへ保存してから、Execution 1個分のTaskをSQSへ送る。'),
node('flow_requests',530,160,'Request queue','Task + input ObjectRefs','sqs','JobId・run_id・Execution・確定入力参照。ファイル本体は載せない。'),
node('flow_worker',880,160,'Worker','trusted supervisor','exec','Taskを受信し、入力の内容を取得・検証して各コンテナへ配置する。'),
node('flow_sandbox',1230,160,'Sandbox session','main + optional peers','exec','Workerが起動するコンテナ群。AWS資格情報・ネットワーク・Docker socketなし。'),
node('flow_worker_result',1230,440,'Worker','same supervisor / publish','exec','mainのJSONを検証。生成物・ログをS3へ保存した後、NodeResultと参照を送る。'),
node('flow_results',880,440,'Result queue','NodeResult + output refs','sqs','結果JSON・終了情報・ObjectRefを運ぶ。Controllerが永続的に受理・適用してから削除。'),
node('flow_controller_accept',530,440,'Controller','same controller / accept','engine','結果inboxと成果物参照を同一transactionで保存し、Engineへ完了を適用。後続入力の参照を解決する。'),
node('flow_db',180,440,'PostgreSQL','inbox + workflow commit','store','参照の永続的な受理とJob状態を保存。結果再適用とCASで後続の二重解放を防ぐ。'),
node('flow_s3',180,740,'S3 inputs','VersionId + SHA-256','s3','指定版を読む。Workerが実際のbyte lengthとSHA-256を照合して実体化する。'),
node('flow_worker_in',530,740,'Worker','download / verify / stage','exec','AWS通信は信頼されたWorkerが担当し、検証済み入力を役割別にsandboxへread-only mountする。'),
node('flow_worker_out',880,740,'Worker','collect / upload','exec','mainの宣言出力とプロセスログを回収し、試行別prefixへ保存する。sandboxはAWS APIを呼ばない。'),
node('flow_outputs',1230,740,'S3 outputs / logs','versioned ObjectRefs','s3','生成ファイルとログを保持。保存した版・ハッシュ・サイズをNodeResultの参照へ入れる。')]
es=[edge('M216 190 H494','Task'),edge('M566 190 H844','receive'),edge('M916 190 H1194','stage / start'),
    edge('M1194 470 H916','NodeResult'),edge('M844 470 H566','receive'),edge('M494 470 H216','durable accept'),
    edge('M216 770 H494','download / verify',kind='data'),edge('M916 770 H1194','upload files',kind='data')]
controls='<div class="panel-controls"><button id="back">戻る</button><button id="next">次へ</button><button id="play">説明を再生</button><input id="step" type="range" min="0" max="7" value="0" aria-label="制御とデータの段階"><span class="sequence-note" id="step-label"></span></div>'
panels.append(panel('flow','03  制御メッセージとファイル転送を別のレーンで読む',[],ns,es,
    '<strong>同名のController / Workerは同じプロセスをレーンごとに再掲。</strong><br>WorkerがS3へ保存 → 結果参照をSQSへ送信 → ControllerがDBへ受理・適用 → その参照を使って次のTaskを送る。',[
    annotation(45,100,'<strong>1　要求 →</strong>　1つのExecutionを起動',1100),
    annotation(45,375,'<strong>2　← 結果</strong>　ファイル保存後に通知、永続受理後にack',1200),
    annotation(45,670,'<strong>3　ファイル内容 →</strong>　SQSを通らず、WorkerだけがS3と転送',1200),
    annotation(350,905,'<strong>mainのstdout JSON と生成ファイルは別。</strong> 後段で計算に使うscore等はArtifactとして宣言する。',1050)],controls))
# One session = one Execution. Main owns both result JSON and declared outputs.
ns=[
node('session_supervisor',155,330,'Worker supervisor','trusted host process','ec2','入力を取得・検証し、mainとpeerを別コンテナで起動。FIFO、制限、終了、回収、cleanupを監督。main終了時に残ったpeerを停止する。','deploy/judge/worker.py'),
node('session_main',620,330,'Interactor / referee','main sandbox','exec','main専用の入力集合を持つ。FIFOでpeerと会話し、stdoutに最終JSONを返す。transcript / replayはmainが宣言出力へ書く。','deploy/judge/worker.py'),
node('session_peer',1210,330,'提出プログラム','peer sandbox','exec','提出コードと起動scriptなど許可したpeer入力だけ。mainの非公開入力、他peerの入力、AWS資格情報、Docker socketは見えない。','deploy/judge/worker.py'),
node('session_result',470,590,'判定JSON','main stdout / NodeResult','output','mainのstdoutは1つのJSON object。Workerが検証してNodeResultに入れる。FIFOの会話やログを混ぜない。','deploy/judge/worker.py'),
node('session_replay',770,590,'transcript / replay','main declared output Artifact','file','mainが書いた宣言出力だけをWorkerが回収し、S3へ保存する。peerのstdoutや出力を自動的にreplayとはしない。','deploy/judge/worker.py')]
es=[edge('M191 360 H304','supervise',250,345),
    edge('M656 350 H1174','from-main → peer',920,337,'channel'),
    edge('M1174 380 H656','to-main → main',920,411,'channel'),
    edge('M600 442 V510 H470 V584','stdout',485,500),
    edge('M640 442 V475 H770 V584','write file',770,465,'data')]
panels.append(panel('session','04  対話は1つのExecution内部のFIFOセッション',[
    group(310,110,1090,720,'ONE Execution / session: each process is a separate container','gb'),
    group(340,210,580,540,'main container / private judge inputs','gb'),
    group(1040,210,330,540,'peer container / submission inputs','ga')],ns,es,
    '<strong>FIFOの双方向通信はDAGの辺にしない。DAGは完成したファイルの依存だけ。</strong><br>coding-gameはpeerを複数持ち、各peerに専用のFIFO対を用意する。コンテナはホストkernelを共有する。',[
    annotation(365,260,'<strong>許可入力</strong>　judge script + 非公開データ',510),
    annotation(1060,260,'<strong>許可入力</strong>　提出 + 起動script',290),
    annotation(365,775,'<strong>Workerが回収・転送：</strong> main stdout → 結果JSON、main outputs → S3。transcript / replayの生成元はmain。',1000),
    annotation(45,880,'<strong>全コンテナ共通：</strong> network none / AWS資格情報なし / UID 65534 / read-only root / capability削除 / no-new-privileges<br>CPU・メモリー・PID・wall time・出力サイズを制限。Docker socketは渡さない。',1340)]))
# Scenario view is compact and readable; the data is the actual executable examples.
scenario_text={
'batch':('通常判定',['compile','binary','run','actual','check'],'Cをコンパイルし、20+22の出力42を確認する。'),
'heuristic':('heuristic',['trial_0','score_0','aggregate','score_1','trial_1'],'独立試行のscoreファイルを集約する。seed 0/1の平均得点は33.0。'),
'run_twice':('run-twice',['run_1','state_1','run_2','state_2','check'],'sandboxを作り直し、宣言したstateだけを次の実行へ引き継ぐ。'),
'interactive':('interactive',['inputs','dialogue','transcript'],'dialogueの内部でinteractorと提出を別sandboxに置く。21→42の会話。transcriptはmainが生成する。'),
'coding_game':('coding-game',['bots','match','replay'],'refereeと2提出を別sandboxに置く。a=4 / b=5、勝者b。replayはmainが生成する。')}
scenario_panels=[]
for kind,(title,names,description) in scenario_text.items():
    sn=[];se=[]
    if kind=='heuristic':
        positions=[(250,220),(570,220),(1020,400),(570,580),(250,580)]
        paths=['M286 250 H534','M606 250 H780 V430 H984','M286 610 H534','M606 610 H780 V450 H984']
        types=['exec','file','exec','file','exec']
    else:
        centers=[170,430,690,950,1210] if len(names)==5 else [270,720,1170]
        positions=[(x,330) for x in centers]
        paths=[f'M{left+36} 360 H{right-36}' for left,right in zip(centers,centers[1:])]
        types=['exec' if n not in ['binary','actual','state_1','state_2','transcript','replay','inputs','bots'] else 'file' for n in names]
    for i,((x,y),name,icon) in enumerate(zip(positions,names,types)):
        sn.append(node('scenario_'+kind+'_'+name,x,y,name,'Execution' if icon=='exec' else ('Input Artifacts (set)' if name in ['inputs','bots'] else 'Artifact'),icon,description,'deploy/judge/examples.py'))
    for path in paths:se.append(edge(path,kind='data'))
    scenario_panels.append(f'<div class="scenario" data-scenario="{kind}"'+(' hidden' if kind!='batch' else '')+'>'+''.join(sn)+f'<svg class="edges" viewBox="0 0 1440 1080"><defs><marker id="s_{kind}" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0 L10 5 L0 10 z" fill="context-stroke"/></marker></defs>'+''.join(se).replace('url(#ar)',f'url(#s_{kind})')+'</svg>'+annotation(80,790,'<strong>'+title+'</strong><br>'+description,1220)+'</div>')
controls='<div class="panel-controls"><label for="scenario">方式</label><select id="scenario">'+''.join(f'<option value="{k}">{v[0]}</option>' for k,v in scenario_text.items())+'</select></div>'
panels.append(panel('scenarios','05  同じファイル／実行モデルで構成する判定方式',[],[],[],'<strong>方式ごとにDAGの基本型を増やす必要はない。</strong>対話・対戦の違いは実行内のsession契約、得点や判定の意味は作問側の処理が担う。',[*scenario_panels],controls))
# Actual AWS proof is loaded only from the redacted verification artifact.
proof=json.loads(a.proof.read_text()) if a.proof and a.proof.exists() else {'aws_verified':False,'checks':{},'scenarios':{}}
verified=proof.get('aws_verified') is True
proof_content='<div class="panel-controls"><label for="proof-scenario">実測した方式</label><select id="proof-scenario">'+''.join(f'<option value="{k}">{k}</option>' for k in proof.get('scenarios',{}))+'</select><span class="status">'+('AWS検証済み' if verified else 'AWS検証未完了')+'</span></div><p class="small">Controllerが保存した実際のdispatch / result_receivedイベント。アカウントID、ARN、実URL、実リソースIDは含めない。</p><div class="proof-wrap" id="proof-table"></div>'
proof_panel='<section class="panel" id="proof" hidden>'+proof_content+'</section>'
steps=[('入力の版・ハッシュを固定',['flow_s3','flow_controller']),('定義とJob依存を保存',['flow_controller','flow_db']),('1ノード分の要求を送る',['flow_controller','flow_requests']),('入力を取得してsandboxを起動',['flow_worker','flow_worker_in','flow_s3','flow_sandbox']),('生成ファイルとログをS3へ保存',['flow_worker_out','flow_outputs']),('結果と生成物参照を送る',['flow_worker_result','flow_results']),('結果inbox・参照を確定して完了を適用',['flow_db','flow_controller_accept']),('前段の生成物参照で後続を依頼',['flow_controller','flow_requests'])]
script='''
const details=DETAILS, proof=PROOF, steps=STEPS;
let step=0,timer=null;
function showPanel(id){document.querySelector('.legend').hidden=id==='aws'||id==='proof';document.querySelectorAll('.panel').forEach(p=>p.hidden=p.id!==id);document.querySelectorAll('[data-panel]').forEach(b=>b.setAttribute('aria-selected',String(b.dataset.panel===id)));document.querySelectorAll('.active').forEach(n=>n.classList.remove('active'));if(id==='flow')setStep(step);}
function setStep(v){step=(Number(v)+steps.length)%steps.length;document.querySelector('#step').value=step;document.querySelector('#step-label').textContent=(step+1)+' / '+steps.length+'  '+steps[step][0];document.querySelectorAll('#flow .node').forEach(n=>n.classList.toggle('active',steps[step][1].includes(n.dataset.id)));}
document.querySelectorAll('[data-panel]').forEach(b=>b.onclick=()=>showPanel(b.dataset.panel));
document.querySelector('#next').onclick=()=>setStep(step+1);document.querySelector('#back').onclick=()=>setStep(step-1);document.querySelector('#step').oninput=e=>setStep(e.target.value);
document.querySelector('#play').onclick=()=>{if(timer){clearInterval(timer);timer=null;}else timer=setInterval(()=>setStep(step+1),1200);};
document.querySelector('#theme').onclick=()=>{let h=document.documentElement;h.hasAttribute('data-theme')?h.removeAttribute('data-theme'):h.setAttribute('data-theme','dark');};
document.querySelector('#scenario').onchange=e=>document.querySelectorAll('[data-scenario]').forEach(s=>s.hidden=s.dataset.scenario!==e.target.value);
document.querySelectorAll('.node').forEach(n=>n.onclick=()=>{const d=details[n.dataset.id],box=document.querySelector('.detail');box.replaceChildren();const title=document.createElement('strong');title.textContent=d.name;const p=document.createElement('p');p.textContent=d.description;const link=document.createElement('a');link.href=d.source;link.textContent='実装・契約を見る';link.target='_blank';link.rel='noopener';box.append(title,p,link);});
function drawProof(name){const box=document.querySelector('#proof-table');box.replaceChildren();const s=proof.scenarios[name];if(!s){box.textContent='実AWSの検証結果はまだありません。';return;}const title=document.createElement('p');title.textContent='Pipeline: '+s.state+' / ケース: '+s.completed_cases+' / '+s.total_cases;box.append(title);const table=document.createElement('table');table.className='trace';const row=(values,head=false)=>{const tr=document.createElement('tr');for(const value of values){const td=document.createElement(head?'th':'td');td.textContent=String(value);tr.append(td);}table.append(tr);};row(['ノード','状態','結果JSON'],true);for(const n of s.nodes)row([n.name,n.state,JSON.stringify(n.result)]);row(['時刻 JST','ノード','観測イベント'],true);for(const e of s.events||[])row([new Date(e.at).toLocaleString('ja-JP',{timeZone:'Asia/Tokyo',hour12:false}),e.node,e.kind+(e.observations>1?' ×'+e.observations:'')]);box.append(table);}
document.querySelector('#proof-scenario').onchange=e=>drawProof(e.target.value);drawProof(document.querySelector('#proof-scenario').value);setStep(0);window.showPanel=showPanel;
'''.replace('DETAILS',json.dumps(details,ensure_ascii=False)).replace('PROOF',json.dumps(proof,ensure_ascii=False)).replace('STEPS',json.dumps(steps,ensure_ascii=False))
body='<div class="page"><header class="head"><div><div class="kicker">traOJudge / AWS / bipartite DAG</div><h1>AWS ジャッジ実行の構成と流れ</h1></div><p class="lede">ファイルの依存、Jobの制御、実行中の会話を切り替えて確認する。ノードを選ぶと責務と実装を表示。</p></header><div class="toolbar" role="tablist">'
for id,label in [('aws','1 AWS全体'),('dag','2 二部DAG'),('flow','3 制御とデータ'),('session','4 実行内の対話'),('scenarios','5 判定方式'),('proof','6 実AWSの記録')]:body+=f'<button data-panel="{id}" role="tab" aria-selected="{str(id=="aws").lower()}">{label}</button>'
body+='</div><button id="theme">Light / Dark</button><p class="small">狭い画面では図を横にスクロールできます。AWS配置図はAWSの矢印記法、他の図は青＝制御・橙＝ファイル・破線＝FIFO会話を使用。</p>'+''.join(panels)+proof_panel+'<div class="legend" hidden><span><span class="sw"></span>制御・起動・結果通知</span><span><span class="sw data-sw"></span>ファイル内容・データ依存</span><span><span class="sw channel-sw"></span>FIFO / session内の会話</span></div><div class="detail" aria-live="polite"><strong>ノードを選択して責務を確認</strong>Artifact、Execution、Job、AWS上のオブジェクトと通信を区別してたどれます。</div><div class="foot"><span>ephemeralなジャッジ経路。公開サービスの認証・問題登録・メールは別の範囲。</span><span>icons: AWS Architecture Icons / Lucide · made with <a href="https://github.com/caru-ini/stack-diagram">stack-diagram</a></span></div></div>'
source='<!DOCTYPE html><html lang="ja"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>AWS ジャッジ実行の構成と流れ</title><style>'+css+'</style></head><body>'+body+'<script>'+script+'</script></body></html>'
a.draft.write_text(source)
with tempfile.TemporaryDirectory(prefix='judge-diagram-') as build_dir:
    built=Path(build_dir)/'diagram.html'
    subprocess.run(['python3',str(a.skill/'scripts/build.py'),str(a.draft),str(a.icons),str(built),'--fonts',str(a.skill/'assets/fonts')],check=True)
    output=built.read_text()
print(json.dumps({'aws_verified':verified,'nodes':len(details),'panels':6}))

# Inline each repeated icon once. The built HTML remains the single screenshot source.
assets={}
def compact_icon(match):
    uri=match[1]
    if uri not in assets: assets[uri]='asset'+str(len(assets))
    return 'data-asset="'+assets[uri]+'"'
output=re.sub(r'src="(data:image/[^" ]+)"',compact_icon,output)
lookup={name:uri for uri,name in assets.items()}
init='<script>const imageAssets='+json.dumps(lookup)+";document.querySelectorAll('[data-asset]').forEach(e=>e.src=imageAssets[e.dataset.asset]);</script>"
output=output.replace('</body>',init+'</body>')
output_bytes=len(output.encode())
if output_bytes > 256 * 1024:
    raise ValueError(f'Self-contained HTML exceeds 256 KiB: {output_bytes} bytes')
a.output.write_text(output)
print('self-contained bytes:',output_bytes)
