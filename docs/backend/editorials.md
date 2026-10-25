# 解説

解説の作成・取得・一覧・編集・削除・公開変更と、編集担当の付与・剥奪を実装している。問題の存在・公開状態・削除状態は、最小構成の `problems` テーブルで検査する。通常起動でも同じPostgreSQLのUoWで参照する。[問題のDB基盤](problems.md)に保存項目と拡張範囲を記載する。

## API

すべて `/api/v1` 配下。既存のセッション認証・Cookie更新と、変更操作のOrigin検証を共通middlewareで適用する。応答は公開取得も含め `Cache-Control: no-store` とする。

| API | 動作 |
| --- | --- |
| `POST /editorials` | `standard-user` が解説を作成。`Idempotency-Key` 必須 |
| `GET /editorials/{editorialId}` | 公開、または本人が編集担当の解説を取得 |
| `GET /problems/{problemId}/editorials` | 閲覧できる問題について、公開解説だけをページ取得 |
| `PATCH /editorials/{editorialId}` | 対象の `editiorial-author` が題名・本文を更新。`If-Match` 必須 |
| `PUT /editorials/{editorialId}/authors/{userId}` | `service-admin` が対象解説の編集担当を付与。既存なら `changed=false` |
| `DELETE /editorials/{editorialId}/authors/{userId}` | `service-admin` が対象の担当だけを解除。解除済みでも204 |
| `DELETE /editorials/{editorialId}` | 投稿者本人またはservice-adminが論理削除 |
| `PUT /editorials/{editorialId}/publication`・`DELETE /editorials/{editorialId}/publication` | 投稿者本人またはservice-adminが公開・非公開化。公開はIf-Match必須 |

暫定合意として、非公開で作成し、投稿者へその解説の編集担当を自動付与する。作問権限や解答の提出は必要ない。投稿者はセッションから決め、本文に含めて指定できない。管理者・問題の管理担当という理由だけでは、解説の編集や非公開本文の閲覧を許可しない。担当解除は投稿者情報や本文を変えず、投稿者本人の削除・公開変更の権限も変えない。

非公開で閲覧権限のない解説、存在しない解説、閲覧できない問題は404とする。題名は1〜200文字、本文は1〜200000文字で、Unicodeの文字数を数える。空の更新・未知フィールド・null・非文字列・NULを拒否する。更新で問題・投稿者・公開範囲を変更できない。

## 責務とUnit of Work

HTTPの `editorials` ルーターは共通の `/api/v1` 配下へ組み込む。共通extractorでセッション・UUID・ETag・JSONを受け、解説専用extractorで冪等キー・ページ要求を読む。生成DTOとの変換とエラー応答は既存のHTTP共通モジュールを使う。

Coreの `EditorialService` が権限・版・入力・再送を判断し、`EditorialUnitOfWorkProvider` から開始したUoWを確定する。InfraはSQLxトランザクションで実装する。本人と担当変更対象の利用者をUUID順にロックしてセッションを再検査し、その後、親の問題を共有ロックし、対象解説をロックする。権限の付与・解除も同じ順序を使う。

作成時の解説・担当の役割・作成結果の記録を同時にcommitする。途中失敗・未確定の破棄・明示的なrollbackでは全変更を破棄する。役割変更に伴う `/me` のrevision更新も同じトランザクションに含まれる。

編集はETagの版確認と保存を同じUoWで行う。同じ版で同時に更新した場合は一方だけが成功し、他方は412。問題の存在・削除状態・閲覧権限も同じUoWで検査する。親の共有ロックで問題の非公開化・削除と解説の取得・作成・更新を直列化する。

## 再送とページング

作成のキーは1〜128文字のASCII英数字と `._:-`。本人と作成操作ごとに独立する。同じキー・同じ入力には、後日本文が編集されていても作成時の表現とETagを返す。異なる入力は409で、新たな解説や担当を作らない。元の解説が削除されていたら再送は404で、過去の作成結果から復活させない。

保存期間は `TRAOJUDGE_EDITORIAL_IDEMPOTENCY_TTL_SECONDS` で指定する正の秒数で、省略時86400秒（24時間）。期限切れのキーは再利用できる。期限切れの記録をまとめて削除する保守処理は今後の課題であり、現在は再利用時にそのキーの記録を置き換える。

一覧は `limit` が1〜100、省略時30。公開解説を作成日時・UUID順に取り、`nextCursor` を返す。カーソルは問題UUID・作成日時・解説UUIDを含む不透明なbase64url文字列で、別の問題には転用できない。編集担当であっても、問題別一覧には非公開解説を含めない。

## 削除と公開範囲

削除・公開・非公開化は、投稿者の記録と現在のservice-adminの役割で判定する。編集担当を任されているだけでは許可しない。公開状態が変われば解説のrevisionとupdatedAtを更新し、本文の取得と同じETagを返す。同じ状態への再要求では版を進めない。公開操作はIf-Matchを検査し、非公開化は既存の契約に従って版の指定なしで204を返す。

解説は `deleted_at` で論理削除し、取得・公開一覧・編集・公開変更・担当管理から除外する。本文・作成結果・過去の担当の記録は保持するが、担当の役割から削除済み解説へアクセスできない。

問題削除時は、その問題に紐づく解説も同時に論理削除する。`problems.deleted_at` の設定をDBトリガーで検出し、同じトランザクションで解説のdeleted_atとrevisionを更新する。問題側の取り下げがrollbackした場合は解説も戻る。APIに `[deleted]` を返す項目は追加しない。

## 問題への参照と今回の範囲

- 公開問題は誰でも参照でき、非公開問題はその問題のproblem-ownerまたはtesterが参照できる。解説が公開されていても親の閲覧権限を検査する。
- 存在しない・削除済み・閲覧できない問題は404。任意のUUIDを有効な問題と扱わず、外部キーでも親の存在を保証する。
- 通常大会経由のcontest-setterの権限は、大会と問題の関連付けを実装する時に追加する。別の問題や別のscopeの役割を流用しない。
- 問題の本文・採点設定・作成/編集APIは後続で追加する。
- 非公開作成・投稿者への編集権限付与は暫定合意とし、コードにもTODOを残す。

レート制限の仕様決定・実装は今回の対象外。役割名 `editiorial-author` は既存のAPI・ロール定義の綴りを維持する。

## 検証

PostgreSQLのテスト専用DBを作成・削除できる接続先で実行する。

```sh
DATABASE_URL=postgresql://traojudge:traojudge@127.0.0.1:5433/traojudge \
  cargo test -p traojudge-test --test editorials -- --include-ignored
```

非公開の保護、担当と投稿者の区別、題名・本文の検証、再送・キーの衝突・期限、公開一覧とページ境界、同時編集、保存途中の失敗・明示rollback・未確定破棄、投稿者・管理者による削除・公開変更、問題と解説の同時削除、実DBの問題参照・scope・外部キー・問題削除との競合を確認する。
