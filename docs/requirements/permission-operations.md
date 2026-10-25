# 操作ごとに、誰が誰に何をしてよいか

[シナリオの一覧](mutation-scenarios.md)にある操作を、許可を判断できる単位まで分ける。各行は「誰が」「誰に／何に」「何をするか」と、成功したときに成立する事実を示す。付与・剥奪、作成・更新・削除、本人による参加・管理者による参加資格の付与を別の操作として扱う。

## 読み方と根拠

- 業務上の許可は[ロール定義](../domain/role.md)を根拠にする。役割の名前だけから上下関係や権限を補わない。
- 認証・本人の認証手段の管理は、[機能要求](14-specified-requirements/02-functions/functions.md)と既存の認証シナリオを根拠にする。本人のプロフィール変更は既存のO2を引き継ぐ。これらを他の人に代行する許可は定義していない。
- Pは問題、Cは通常の大会、Vはバーチャル大会、Eは解説を表す。同じ人でも、P1の担当であることを理由にP2を変更できるとはしない。
- `contest-setter(C)` は、現在Cに含まれる問題に限り `tester(P)` と同じ操作を行える。バーチャル大会の運営担当にこの許可はない。
- `service-admin` は「すべての操作ができる人」ではない。たとえば問題の削除をするには、その問題の `problem-owner` を持つ必要がある。役割を自分に付与できるかは別途未決であり、自動で補わない。
- 「未決」は、担当者が任意に許可してよいという意味ではない。成功する操作の実装前に条件を決める。表中の結果も、その条件を決めた後の到達点である。
- 複数の役割を持つ場合の評価、途中で役割を外した場合の反映時点、作成者への役割の自動付与は未決。認証方法を追加しても役割は増えない。
- ロール定義の解説編集者の綴りは `editiorial-author`。ここでは参照元に合わせ、役割を新しく増やさない。

各リンク先には利用者の言葉で書いた流れとMermaidを置く。ファイルが同じでも、表の行ごとに許可を確認する。読み取りAPIとクラスタ内部の通信方式はここでは決めない。

## 問題投稿

| 操作 | 何をするか | 誰がしてよいか | 誰に／何に・範囲 | 成立する事実 |
| --- | --- | --- | --- | --- |
| P1-01 | [新しい問題を作る](scenarios/problems/create-problem.md) | 作問する資格を持つ人（writer） | 新規の問題 | 非公開の問題ができた |
| P1-02 | [題名・問題文を編集する](scenarios/problems/create-problem.md) | 問題Pの problem-owner / tester、またはPを含む通常大会Cの contest-setter | その問題P | 問題の内容が変わった |
| P1-03 | [問題を削除する](scenarios/problems/create-problem.md) | 問題Pの problem-owner | その問題P | 問題を取り下げた |
| P2-01 | [CLIで登録を始め、送り先を受け取る](scenarios/problems/upload-problem-files.md) | 本人の有効な作問ツールの認証手段を使う人。下記の各変更の許可も必要 | 対象の問題Pと、今回登録する内容 | 今回の登録が始まった |
| P2-02 | [CLIでファイルを送り、登録を完了する](scenarios/problems/upload-problem-files.md) | 登録を始めた本人。問題・採点方法それぞれの変更の許可が必要 | 自分が開始した登録と問題P | ファイル・メタデータ・参照の検証を通り、今回の版が決まった |
| P3-01 | [テストを追加する](scenarios/problems/edit-tests-and-judging.md) | 未決。問題の更新に含めるかを role.md だけでは特定できない | 問題Pのテスト | テストが増えた |
| P3-02 | [テストを編集する](scenarios/problems/edit-tests-and-judging.md) | 未決。問題の更新に含めるかを role.md だけでは特定できない | 問題Pの指定したテスト | テストの内容が変わった |
| P3-03 | [テストを削除する](scenarios/problems/edit-tests-and-judging.md) | 未決。問題の更新に含めるかを role.md だけでは特定できない | 問題Pの指定したテスト | テストを取り除いた |
| P3-04 | [採点方法を追加する](scenarios/problems/edit-tests-and-judging.md) | 問題Pの problem-owner | その問題P | 採点方法が増えた |
| P3-05 | [採点方法を更新する](scenarios/problems/edit-tests-and-judging.md) | 問題Pの problem-owner / tester、またはPを含む通常大会Cの contest-setter | 問題Pに属する、指定した採点方法 | 採点方法の内容が変わった |
| P3-06 | [採点方法を削除する](scenarios/problems/edit-tests-and-judging.md) | 問題Pの problem-owner、または service-admin | 指定した採点方法。service-admin は全問題のものが対象 | 採点方法を削除した。service-admin による削除は記録を残す |

## 提出

| 操作 | 何をするか | 誰がしてよいか | 誰に／何に・範囲 | 成立する事実 |
| --- | --- | --- | --- | --- |
| S1-01 | [大会とは別に解答を提出する](scenarios/submissions/submit-answer.md) | 基本的な利用資格を持つ人（standard-user） | 公開された問題Pに、自分の解答を提出 | 自分の提出が受け付けられた |
| S1-02 | [通常の大会に解答を提出する](scenarios/submissions/submit-answer.md) | 通常大会Cの contestant | その大会Cに含まれる問題Pに、自分の解答を提出。Cの開催時間中に限る | 大会Cへの自分の提出が受け付けられた |
| S1-03 | [バーチャル大会に解答を提出する](scenarios/submissions/submit-answer.md) | バーチャル大会Vの virtual-contestant | その大会Vに含まれる問題Pに、自分の解答を提出。Vの開催時間中に限る | 大会Vへの自分の提出が受け付けられた |
| S2-01 | [提出を採点し直す](scenarios/submissions/judge-again.md) | 問題Pの problem-owner / tester、またはPを含む通常大会Cの contest-setter | その問題Pに対する提出。提出した本人である必要はない | 今回の採点し直しを受け付けた |
| S3-01 | [通常の大会の提出を問題ごとに一括で採点し直す](scenarios/submissions/judge-many-again.md) | 通常大会Cの contest-admin | 大会Cの各問題に対する、大会Cの提出 | 大会Cの範囲の一括採点し直しを受け付けた |
| S3-02 | [問題単位で全提出を一括で採点し直す](scenarios/submissions/judge-many-again.md) | 未決。problem-owner / tester の rejudge が一括操作も含むか確認が必要 | 問題Pに対する全提出 | 対象の範囲を固定して受け付けた |

## コンテスト

| 操作 | 何をするか | 誰がしてよいか | 誰に／何に・範囲 | 成立する事実 |
| --- | --- | --- | --- | --- |
| C1-01 | [通常の大会を作る](scenarios/contests/organize-contest.md) | サービス管理者（service-admin） | 新規の通常大会 | 通常の大会ができた |
| C1-02 | [通常の大会のルール・開始終了時刻・参加方式を変更する](scenarios/contests/organize-contest.md) | 通常大会Cの contest-setter | その大会C。参加方式は開始時刻より前に限る | 大会Cの設定が変わった。方式変更時は既存の参加登録と大会内チームが削除された |
| C1-03 | [通常の大会を削除する](scenarios/contests/organize-contest.md) | サービス管理者（service-admin） | 指定した通常大会C | 大会Cを取り下げ、記録を残した |
| C1-04 | [バーチャル大会を作る](scenarios/contests/organize-contest.md) | 基本的な利用資格を持つ人（standard-user） | 新規のバーチャル大会 | バーチャル大会ができた |
| C1-05 | [バーチャル大会のルール・開始終了時刻を変更する](scenarios/contests/organize-contest.md) | バーチャル大会Vの virtual-contest-setter | その大会V | 大会Vの設定が変わった |
| C1-06 | [バーチャル大会を削除する](scenarios/contests/organize-contest.md) | 未決。service-admin の contest 削除に virtual-contest も含むか明記がない | 指定したバーチャル大会V | 大会Vを取り下げた |
| C2-01 | [通常の大会に問題を追加する](scenarios/contests/choose-problems.md) | 通常大会Cの contest-setter | 大会Cに、自身の問題Pを追加。「自身」の判定基準は未決 | 問題Pが大会Cの出題に加わった |
| C2-02 | [通常の大会から問題を外す](scenarios/contests/choose-problems.md) | 通常大会Cの contest-setter | 大会Cに含まれる任意の問題P。自分の問題に限らない | 大会Cとの結び付きだけが外れた |
| C2-03 | [バーチャル大会に問題を追加する](scenarios/contests/choose-problems.md) | バーチャル大会Vの virtual-contest-setter | 大会Vに、公開された問題Pを追加。自分の問題に限らない | 問題Pが大会Vの出題に加わった |
| C2-04 | [バーチャル大会から問題を外す](scenarios/contests/choose-problems.md) | 未決。virtual-contest-setter に明示された操作ではない | 大会Vに含まれる問題P | 大会Vとの結び付きだけが外れた |
| C3-01 | [通常の大会に質問する](scenarios/contests/ask-and-answer.md) | 通常大会Cの contestant | その大会Cに、自分の質問を投稿 | 大会Cへの質問が残った |
| C3-02 | [通常の大会の質問に回答する](scenarios/contests/ask-and-answer.md) | 通常大会Cの contest-setter | 大会Cに寄せられた参加者の質問。回答とともに大会参加者への公開可否を指定 | 回答と選んだ公開範囲が保存された |

## 認証

| 操作 | 何をするか | 誰がしてよいか | 誰に／何に・範囲 | 成立する事実 |
| --- | --- | --- | --- | --- |
| A1-01 | [メールで利用登録を申し込む](scenarios/authentication/register.md) | そのメールを受け取れる本人 | 自分のメールの本人確認 | 登録の有無によらず同じ202で共通の本人確認メールの送信を受け付け、API応答とメールでは登録の有無を開示しない |
| A1-02 | [メールを確認する](scenarios/authentication/register.md) | 有効な本人確認トークンを持つ本人 | 自分のメールに紐付く登録・ログインへの分岐 | メール確認後にだけ、登録済みならログインし、未登録なら登録用トークンを得た |
| A1-03 | [登録を完了する](scenarios/authentication/register.md) | メール確認後に発行された有効な登録用トークンを持つ本人 | 自分の利用登録 | 登録され、standard-user になり、本人としてログインできた |
| A2-01 | [メールの本人確認リンクを頼む](scenarios/authentication/sign-in-and-out.md) | そのメールを受け取れる本人 | 自分の登録・ログインに向けた本人確認 | 登録の有無によらず同じ202で共通の本人確認メールの送信を受け付け、API応答とメールでは登録の有無を開示しない |
| A2-02 | [メールを確認し登録・ログインへ進む](scenarios/authentication/sign-in-and-out.md) | 有効な本人確認トークンを持つ本人 | 自分のメールに紐付く登録・ログインへの分岐 | メール確認後にだけ、登録済みならログインし、未登録なら新規登録へ進めるようになった |
| A2-03 | [ログアウトする](scenarios/authentication/sign-in-and-out.md) | ログインしている本人 | 要求のCookieで特定される自分の現在のセッションだけ | 現在のセッションが失効し、Cookieが削除された。ほかの端末や別のブラウザのセッションは維持される |
| A3-01 | [新しいメールアドレスの確認を頼む](scenarios/authentication/change-email.md) | ログインしている本人 | 自分の登録先メール | 新しいアドレスの確認が始まった |
| A3-02 | [新しいメールを確認し、変更を完了する](scenarios/authentication/change-email.md) | 新しいメールを確認した本人。開始した利用者との一致が必要 | 自分の登録先メール | 登録先が切り替わった |
| A4-01 | [作問ツール用の認証手段を発行する](scenarios/authentication/use-authoring-tool.md) | ログインしている本人 | 自分の作問ツール用の認証手段 | 自分の名前でツールを使えるようになった |
| A4-02 | [作問ツール用の認証手段を無効にする](scenarios/authentication/use-authoring-tool.md) | ログインしている本人 | 自分が持つ指定した認証手段 | その認証手段を使えなくした |
| A5-01 | [外部アカウントで利用登録する](scenarios/authentication/register-with-another-service.md) | 未登録で、外部サービスで本人確認ができた本人 | 新規の自分の利用登録 | 登録され、standard-user になり、本人としてログインできた |
| A6-01 | [外部アカウントでログインする](scenarios/authentication/sign-in-with-another-service.md) | 登録済みの外部アカウントで本人確認ができた本人 | その外部アカウントに対応する自分の利用者 | 本人としてログインできた |
| A7-01 | [ログイン方法を追加する](scenarios/authentication/add-sign-in-method.md) | ログイン中で、追加する外部アカウントでも本人確認できた本人 | 自分のログイン方法。ほかの利用者に結び付いた方法は使わない | 同じ利用者にログイン方法が増えた |
| A8-01 | [ログイン方法を外す](scenarios/authentication/remove-sign-in-method.md) | ログインしている本人 | 自分のログイン方法。最後のWebログイン方法は外さない | 指定した方法が外れた |

## 権限付与／剥奪・参加・公開範囲

| 操作 | 何をするか | 誰がしてよいか | 誰に／何に・範囲 | 成立する事実 |
| --- | --- | --- | --- | --- |
| R1-01-G | [作問する資格を任せる](scenarios/permissions/grant-writer.md) | サービス管理者（service-admin） | 登録済みの対象者1人、サービス全体 | 対象者に writer が付いた |
| R1-01-D | [作問する資格を外す](scenarios/permissions/revoke-writer.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、サービス全体 | 対象者から writer が外れた |
| R1-02-G | [問題の管理担当を任せる](scenarios/permissions/grant-problem-owner.md) | サービス管理者（service-admin） | 登録済みの対象者1人、選んだ問題1つ | 対象者に problem-owner が付いた |
| R1-02-D | [問題の管理担当を外す](scenarios/permissions/revoke-problem-owner.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、選んだ問題1つ | 対象者から problem-owner が外れた |
| R1-03-G | [問題のテスト担当を任せる](scenarios/permissions/grant-tester.md) | サービス管理者（service-admin）、またはその問題の管理担当（problem-owner） | 登録済みの対象者1人、選んだ問題1つ | 対象者に tester が付いた |
| R1-03-D | [問題のテスト担当を外す](scenarios/permissions/revoke-tester.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、選んだ問題1つ | 対象者から tester が外れた |
| R1-04-G | [通常の大会の責任者を任せる](scenarios/permissions/grant-contest-admin.md) | サービス管理者（service-admin） | 登録済みの対象者1人、選んだ通常の大会1つ | 対象者に contest-admin が付いた |
| R1-04-D | [通常の大会の責任者を外す](scenarios/permissions/revoke-contest-admin.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、選んだ通常の大会1つ | 対象者から contest-admin が外れた |
| R1-05-G | [通常の大会の運営担当を任せる](scenarios/permissions/grant-contest-setter.md) | サービス管理者（service-admin）、またはその大会の責任者（contest-admin） | 登録済みの対象者1人、選んだ通常の大会1つ | 対象者に contest-setter が付いた |
| R1-05-D | [通常の大会の運営担当を外す](scenarios/permissions/revoke-contest-setter.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、選んだ通常の大会1つ | 対象者から contest-setter が外れた |
| R1-06-G | [好きな時間に挑戦する大会の運営担当を任せる](scenarios/permissions/grant-virtual-contest-setter.md) | サービス管理者（service-admin） | 登録済みの対象者1人、選んだバーチャル大会1つ | 対象者に virtual-contest-setter が付いた |
| R1-06-D | [好きな時間に挑戦する大会の運営担当を外す](scenarios/permissions/revoke-virtual-contest-setter.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、選んだバーチャル大会1つ | 対象者から virtual-contest-setter が外れた |
| R1-07-G | [基本的な利用資格を任せる](scenarios/permissions/grant-standard-user.md) | サービス管理者（service-admin） | 登録済みの対象者1人、サービス全体 | 対象者に standard-user が付いた |
| R1-07-D | [基本的な利用資格を外す](scenarios/permissions/revoke-standard-user.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、サービス全体 | 対象者から standard-user が外れた |
| R1-08-G | [解説の編集担当を任せる](scenarios/permissions/grant-editorial-author.md) | サービス管理者（service-admin） | 登録済みの対象者1人、選んだ解説1つ | 対象者に editiorial-author が付いた |
| R1-08-D | [解説の編集担当を外す](scenarios/permissions/revoke-editorial-author.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、選んだ解説1つ | 対象者から editiorial-author が外れた |
| R1-09-G | [通常の大会の参加資格を任せる](scenarios/permissions/grant-contestant.md) | サービス管理者（service-admin） | 登録済みの対象者1人、選んだ通常の大会1つ | 対象者に contestant が付いた |
| R1-09-D | [通常の大会の参加資格を外す](scenarios/permissions/revoke-contestant.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、選んだ通常の大会1つ | 対象者から contestant が外れた |
| R1-10-G | [好きな時間に挑戦する大会の参加資格を任せる](scenarios/permissions/grant-virtual-contestant.md) | サービス管理者（service-admin） | 登録済みの対象者1人、選んだバーチャル大会1つ | 対象者に virtual-contestant が付いた |
| R1-10-D | [好きな時間に挑戦する大会の参加資格を外す](scenarios/permissions/revoke-virtual-contestant.md) | サービス管理者（service-admin） | 役割を持つ対象者1人、選んだバーチャル大会1つ | 対象者から virtual-contestant が外れた |
| R1-11 | [サービス管理者を任命・解任する](scenarios/permissions/change-permissions.md) | 未決。service-admin は自分と同じ役割を付与・剥奪できない | 登録済みの対象者1人 | サービス管理者の構成が変わった |
| R2-01 | [問題を公開する](scenarios/permissions/publish-problem.md) | 未決 | 問題P | 問題Pが公開された |
| R2-02 | [問題を非公開にする](scenarios/permissions/hide-problem.md) | サービス管理者（service-admin）。他の役割は未決 | 担当を問わず、指定した問題P | 問題Pが非公開になった |
| R2-03 | [解説を公開する](scenarios/permissions/publish-editorial.md) | 未決 | 解説E | 解説Eが公開された |
| R2-04 | [解説を非公開にする](scenarios/permissions/hide-editorial.md) | 未決 | 解説E | 解説Eが非公開になった |
| R3-01 | [公開された通常の大会に参加を申し込む](scenarios/permissions/join-contest.md) | standard-user 本人 | 自分自身、公開された通常大会C | 個人戦では本人が大会Cの contestant になり、チーム戦では本人が大会C内の1チームに所属する。チームの移動・脱退は大会開始前まで可能 |
| R4-01 | [バーチャル大会に参加を申し込む](scenarios/permissions/join-virtual-contest.md) | 未決。通常の大会への自己参加と同じ許可は明記されていない | 自分自身、バーチャル大会V | 本人が大会Vの virtual-contestant になった |

## それ以外

| 操作 | 何をするか | 誰がしてよいか | 誰に／何に・範囲 | 成立する事実 |
| --- | --- | --- | --- | --- |
| O1-01 | [解説を投稿する](scenarios/others/write-editorial.md) | 基本的な利用資格を持つ人（standard-user） | 問題Pに、新しい自分の解説 | 解説Eができた |
| O1-02 | [解説を編集する](scenarios/others/write-editorial.md) | 解説Eの編集担当（参照元の表記は editiorial-author） | その解説E | 解説Eの内容が変わった |
| O1-03 | [解説を削除する](scenarios/others/write-editorial.md) | 未決。編集の許可に削除を含めない | 解説E | 解説Eを取り下げた |
| O2-01 | [プロフィールを書き換える](scenarios/others/edit-profile.md) | ログインしている本人 | 自分のプロフィール。ほかの人のプロフィールは対象外 | 自分のプロフィールが変わった |
| O3-01 | [公開プロフィールを取得する](scenarios/others/view-user-profile.md) | 誰でも | 指定利用者の公開プロフィール | 表示名と自己紹介を確認できた |
| O3-02 | [作成した閲覧可能な問題を取得する](scenarios/others/view-user-profile.md) | 誰でも。ログイン時は権限のある非公開問題も含む | 指定利用者が作成者として記録され、閲覧者が閲覧できる問題 | 閲覧可能な問題の一覧を確認できた |

## 読み取りについて、既に分かっている許可

読み取りAPIは後で考えるが、ロール定義にある閲覧権限を落とさないために記す。

| 誰が | 誰の何を見てよいか | 範囲 |
| --- | --- | --- |
| ログインしていない人も含む利用者 | 公開された情報 | 公開されているもの。通常大会・バーチャル大会の終了時刻に到達した後は、その大会に紐づく提出ソースも公開情報となる |
| 提出者本人（今回の進捗表示の対象） | 自分の提出の、今回の採点で完了したテストケース数 | [結果を待つシナリオ](scenarios/submissions/watch-judging.md)。総数・ケース名・判定の詳細や他人の提出の進捗の公開範囲は別に決める |
| 提出者本人 | 自分の提出ソース | いつでも `/submissions/{submissionId}/source` から取得できる。APIはS3のファイルごとの署名付きURLを発行する |
| 通常大会のチーム戦の contestant | 同じチームの全メンバーの提出情報・採点結果・提出ソース | 提出一覧は `/contests/{contestId}/teams/{teamId}/submissions`、提出ソースは `/submissions/{submissionId}/source` から取得する。非公開テストは含めない |
| ログイン有無を問わない全利用者 | 終了時刻に到達した通常大会・バーチャル大会の提出情報と提出ソース | 大会別提出一覧と `/submissions/{submissionId}/source` から取得する。非公開問題への提出も大会に紐づくものは対象に含める。非公開テストは含めない |
| 問題Pの problem-owner / tester | 問題P | その問題のみ |
| 通常大会Cの contest-setter | 大会Cに含まれる問題P | tester と同じ。大会外の問題には及ばない |
| 通常大会Cの contest-setter | 大会Cでの、他の参加者を含む提出 | 大会中の提出の閲覧。大会外の提出には及ばない |
| バーチャル大会Vの virtual-contest-setter | 大会Vでの、他の参加者を含む提出 | 大会中の提出の閲覧。出題した問題そのものへの tester 相当の許可はない |

Clarやテストの見せ方の詳細は未決。読み取りの権限から更新の権限もあるとは扱わない。

## 単独で確認する例

- 問題P1の管理担当はP1のテスト担当を追加でき、P2ではできない。P1のテスト担当を外す操作もできない。
- 通常大会C1の責任者はC1の運営担当を追加できる。C2での追加、C1での取り消し、C1のルール変更は、責任者という役割だけではできない。
- テスト担当は採点方法の更新ができ、追加・削除はできない。CLIで一式を送る場合も同じ判定をする。
- 通常大会C1の運営担当は、C1の問題を編集できる。C1から外れた後に、この大会の役割だけで編集を続ける許可はない。変更途中で外れたときの反映時点は未決。
- 通常大会C1の運営担当は、ほかの人の問題もC1から外せるが、その問題自体を削除する許可は得ない。
- バーチャル大会V1の運営担当は、公開問題Pを出題に追加できるが、Pの採点方法を更新する許可は得ない。
- 基本的な利用資格を持つ人は、公開された通常大会に自分を参加させられる。ほかの人を参加させるには別の許可が必要。
- 外部アカウントの追加、CLIの認証手段の発行だけでは、作問する資格や問題の担当は増えない。

## 未決事項の所在

権限変更の各シナリオと、上表で未決とした操作を合意する。[実装メモのMUT-02・MUT-07・MUT-09](scenario-implementation-notes.md#作業票の形式と未決事項)にも対応付ける。役割定義を変更した場合は、この表・該当シナリオ・確認例を一緒に更新する。
