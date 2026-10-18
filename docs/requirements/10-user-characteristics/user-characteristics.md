# ユーザー特性

特定コミュニティへの所属を利用条件としない一般公開サービスとする。利用者は管理者承認なしで登録でき、登録直後に `standard-user` となる。

| 利用者・ロール | 主な利用目的 |
| --- | --- |
| 非ログイン利用者 | 公開情報を閲覧する |
| standard-user | 公開問題への提出、解説投稿、virtual-contestの作成、コンテストへの参加 |
| writer / problem-owner | 問題の作成、所有する問題とジャッジ方法の管理 |
| tester | 権限を与えられた問題の更新・検証・再ジャッジ |
| contest-admin / contest-setter | 通常コンテストの運営・問題管理・問い合わせ対応 |
| virtual-contest-setter | 公開問題を使ったvirtual-contestの運営 |
| contestant / virtual-contestant | 対象コンテストへの提出とClar |
| editiorial-author（ロール定義の表記） | 自分が投稿した解説の編集 |
| service-admin | サービス全体に関わるロール管理やコンテンツ管理 |

これは利用目的の整理であり、詳細な権限は[ロール定義](../../domain/role.md)による。問題・コンテストなどに紐付くロールを、全対象への権限として扱わない。

- TODO USER-01: 個人参加とチーム参加の対応範囲を決める。
- TODO USER-02: 想定利用者数、作問者数、コンテスト規模を決める。
- TODO USER-03: 初期service-adminの設定、service-admin自体の追加・剥奪、退会時の権限の扱いを決める。
