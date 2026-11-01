# 通常大会・バーチャル大会

管理・出題・参加をDBからHTTPまで実装している。提出・採点・順位表・Clar・再採点は後続の実装対象。

## APIと権限

すべて `/api/v1` 配下。通常大会は `/contests`、バーチャル大会は `/virtual-contests` にまとめる。既存のセッション認証・Cookie更新・Origin検証を共通middlewareで適用し、応答は `Cache-Control: no-store` とする。

| 操作 | 通常大会 | バーチャル大会 |
| --- | --- | --- |
| 作成 `POST /` | service-admin。役割は自動付与しない | standard-user。作成者へvirtual-contest-setterを自動付与 |
| 公開一覧 `GET /` | 公開・有効な大会だけ | 同左 |
| 設定取得 `GET /{id}` | 公開、またはcontest-admin・contest-setter | 公開、またはvirtual-contest-setter |
| 設定更新 `PATCH /{id}` | contest-setter | virtual-contest-setter |
| 論理削除 `DELETE /{id}` | service-admin | virtual-contest-setter |
| 出題追加・取り外し `PUT /{id}/problems/{problemId}`・`DELETE` | contest-setter。追加は本人が作成者の問題だけ | virtual-contest-setter。追加は公開問題だけ |
| 出題一覧 `GET /{id}/problems` | 開始前はcontest-admin・contest-setterだけ | 開始前はvirtual-contest-setterだけ |
| 個人参加 `POST /{id}/participation` | standard-user、公開された個人戦 | standard-user、公開大会 |
| チーム作成・参加・移動 `POST /{id}/teams` | standard-user、公開されたチーム戦 | なし |
| チーム取得 `GET /{id}/teams/{teamId}` | 対象大会のcontestant・contest-setter | なし |
| チーム脱退 `DELETE /{id}/participation` | standard-user、チーム戦、本人の所属だけ | なし |

service-adminは `PUT /{id}/admins/{userId}` で通常大会のcontest-adminを任命する。contest-adminまたはservice-adminは `PUT /{id}/setters/{userId}` でcontest-setterを任命できる。バーチャル大会のsetterと両大会のparticipantの手動付与はservice-adminだけが行う。各割り当ての解除は同じURLの `DELETE` で、service-adminだけが行う。役割は本人・種類・大会UUIDをすべて一致させ、別の大会や別の役割へ広げない。

問題の「自身」は `problems.created_by_user_id` と本人の一致で判定する。problem-ownerの割り当ては作成者の記録を変えない。既存の作成者不明データはNULLとして保持し、担当から推測しない。

開始後の出題一覧は、公開大会なら誰でも取得でき、非公開大会なら運営担当・参加者に限定する。問題UUID・ラベル・順序だけを返し、問題本文を開示しない。削除済み問題は一覧から除外するが、出題枠の記録は保持する。

## 時刻と設定の境界

- 題名・公開範囲・開始時刻・ルール・通常大会の参加方式は、更新前の開始時刻以降は変更できない。開始前は対象大会の運営担当が、既存のPATCH APIで題名・公開範囲も変更できる。開始後は終了時刻と通常大会の概要だけを変更できる。
- 出題の追加・取り外しと、新規の個人参加は、現在の終了時刻より前まで受け付ける。終了時刻ちょうどは受け付けない。
- チームの新規参加・移動・脱退は、現在の開始時刻より前だけ。同じチームへの再要求は開始後も現在の所属を返す。
- 参加方式を開始前に変更すると、その大会の参加者の役割・チーム・所属を同じトランザクションで削除する。運営担当の役割は残す。
- バーチャル大会も全員が共通の開始・終了時刻を使い、個人別開始・再挑戦は設けない。異なる再送キーで参加を要求しても、登録済みなら同じ参加状態を返す。
- 題名・公開範囲の変更では既存の参加資格やチームをリセットしない。非公開化は公開一覧・匿名取得へ即座に反映する。

設定更新は `If-Match` が必須で、欠落は428、不一致は412。空の更新・未知フィールド・null・不正な入れ子を拒否する。開始終了の前後関係は保存先と同じマイクロ秒精度で検査し、ルールの非負値も検査する。概要はMarkdownで、raw HTMLを拒否する。コード中のHTML表記は許可する。

出題の省略時の順序は、未取り外しの出題枠の最大値+1。ラベルはその順序からA、B、…、Z、AA、…とする。同じ問題・同じ設定の再要求は現在の関連を返す。既存のラベル・順序を暗黙に上書きせず、異なる設定や重複は409とする。取り外しは問題そのものを削除しない。

## Unit of Workと保存

Coreの `ContestService` が権限・状態遷移・commitを判断し、`ContestUnitOfWorkProvider` / `ContestUnitOfWork` を通して永続化する。HTTPはextractor・ルーティング・生成DTOへの変換を担当し、SQLはInfraへ置く。

PostgreSQLのUoWは対象大会を先にロックし、影響する利用者をUUID順にロックしてセッションの失効を再検査する。時刻判定はロック取得後のDB時計を使う。参加方式を変更するときは全参加者も同じ順序でロックする。役割変更に伴う `/me` のrevision更新、参加状態、再送記録をまとめて確定し、途中失敗ではすべてrollbackする。

`contests` は大会の種類・設定・作成者・版・削除日時を持つ。`contest_problems` は出題、`contest_teams` と `contest_team_memberships` は大会内のチームと所属を持つ。参加資格は `account_roles` を正として、チーム所属は大会・本人ごとに一意とする。外部キーで別大会のチームに所属できないようにする。

同じ大会内のチーム名・パスワードの組み合わせが同じなら既存チームに参加し、異なれば新しいチームを作る。同名で異なるパスワードのチームは別チーム。パスワードは大会のランダムなsaltとチーム名を使うArgon2idのPHC形式で保存する。ハッシュ計算中はUoWを保持せず、完了後に認証・権限・時刻・参加方式を再検査する。平文・ハッシュ・saltはHTTP応答へ含めない。

通常大会が有効で出題されている間、その大会のcontest-setterは親問題の閲覧権限を継承し、解説の親問題の検査にも使う。大会の論理削除・出題の取り外し後には継承しない。解説の編集担当や、非公開解説そのものの閲覧権限を付与するものではない。

## 再送と一覧

作成・個人参加・チーム参加には共通の `Idempotency-Key` extractorを使う。キーは本人・操作・対象大会ごとに独立する。同じキーの異なる入力は409。作成の再送は作成時の表現・ETagを返すが、削除済み大会を復活させない。チーム参加の再送は同じ所属のままの場合だけ受け付け、メンバー一覧は現在のものを返す。脱退・移動・参加資格の再付与や方式変更をまたぐ古い参加結果を再利用しない。

再送記録の保存期間は `TRAOJUDGE_CONTEST_IDEMPOTENCY_TTL_SECONDS` の正の秒数。省略時は86400秒（24時間）。期限切れは同じキーの再利用時に置き換える。

公開一覧は作成日時・UUID順。`limit` は1〜100、省略時30。`nextCursor` は大会の種類にも結び付け、通常大会とバーチャル大会の間で使い回せない。

## 後続で扱うこと

- 問題本文・版・作成編集APIと、開催中の本文変更・版固定の仕様。
- 個人参加の自己取り消し。
- 提出・順位表・Clar・再採点との接続。
- 期限切れ再送記録の定期削除。レート制限の仕様決定・実装。

問題の版固定は問題基盤の実装時に接続する。未実装のAPIを503や仮の成功で補わない。

## 検証

PostgreSQLでテスト専用DBの作成権限がある接続先を指定する。

```sh
DATABASE_URL=postgresql://postgres:test@127.0.0.1:55432/postgres \
  cargo test -p traojudge-test --test contests -- --include-ignored
```

権限・スコープ・HTTP共通処理・再送・時刻制約・同時チーム参加・方式変更の全消去・保存途中の失敗・セッション失効・解説への親問題の閲覧権限の継承を確認する。
