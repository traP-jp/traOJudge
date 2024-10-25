// 登録するEmailのルール
pub const EMAIL_RULE: &str = r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$";

// ユーザー名のルール
pub const USER_NAME_RULE: &str = r"^[a-zA-Z0-9](?:[a-zA-Z0-9_-]{0,30}[a-zA-Z0-9])?$";

// パスワードのルール
pub const PASSWORD_RULE: &str = r"^(?=.*[a-z])(?=.*[A-Z])[a-zA-Z0-9@$!%*?&]{8,64}$";

// アイコンのルール
pub const ICON_RULE: &str = r".{1,25565}";

// X の link のルール
pub const X_LINK_RULE: &str = r"(?:https?://)?(?:www\.)?(twitter|x)\.com/[a-zA-Z0-9_/]+";

// GitHub の link のルール
pub const GITHUB_LINK_RULE: &str = r"(?:https?://)?(?:www\.)?github\.com/[a-zA-Z0-9_-]+";

// ユーザーの自己紹介のルール
pub const SELF_INTRODUCTION_RULE: &str = r".{0,10000}";
