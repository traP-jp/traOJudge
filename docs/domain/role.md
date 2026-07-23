# コンテスト周りのあれこれの議論

## service-admin

### 権限
- contestを作成できる
- contestを論理削除できる
- service-admin以外のroleの付与/剥奪ができる
- 全てのproblemをprivateにできる
- 全てのjudge-definitionを論理削除できる


## contest-admin (ContestId contest_id)

### 権限
- userをcontest-setterにする (削除はservice-adminに依頼する)
- contestの問題それぞれについて、提出を一括でrejudgeできる


## contest-setter (ContestId contest_id)

### 権限
- contestに自身のproblemを追加できる
- contest内の全ての問題をcontestから削除できる
- contest内のすべての問題に対して、testerと同等の権限を持つ
- contestのルールや開催時間終了時間の決定・update
- contest中の提出の閲覧とClarへの返答


## writer

### 権限
- problemを作成できる


## problem-owner (ProblemId problem_id)

### 権限
- problemを読み取り/更新/削除できる
- rejudgeできる
- judge-definitionを追加/更新/削除できる
- testerを追加できる


## tester (ProblemId problem_id)

### 権限
- problemを読み取り/更新できる
- rejudgeできる
- judge-definitionを更新できる


## virtual-contest-setter (VirtualContestId virtual_contest_id)

### 権限
- publicな問題を紐づけることができる
- contestのルールや開催時間終了時間の決定・update
- contest中の提出の閲覧とClarへの返答


## standard-user

### 権限
- virtual-contestを作成できる
- publicな問題に対して、提出をできる
- publicなコンテストに対して、contestantになることができる
- editorialを追加できる


## editiorial-author (EditorialId editorial_id)

### 権限
- editorialの編集をできる


## contestant (ContestId contest_id)

### 権限
- contest内の問題に対して、コンテスト時間中に提出できる
- contestに対してClarを送信できる

## virtual-contestant (VirtualContestId virtual_contest_id)
### 権限
- virtual-contest内の問題に対して、コンテスト時間中に提出できる
- virtual-contestに対してClarを送信できる



## 非ログイン
pubic なものを閲覧可能

---

ドキュメントにするならこういうマトリックスを生やしたほうがよさそうな
https://gitlab.com/gitlab-org/gitlab/-/blob/master/doc/user/permissions.md