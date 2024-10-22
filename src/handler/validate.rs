use regex::Regex;

const PASSWORD_REGEX: &str = r"^(?=.*[a-z])(?=.*[A-Z])[a-zA-Z0-9@$!%*?&]{8,64}$";
const USERNAME_REGEX: &str = r"^[a-zA-Z0-9_-]{3,16}$";

pub fn is_valid_password(password: &str) -> bool {
    let re = Regex::new(PASSWORD_REGEX).unwrap();
    re.is_match(password)
}

pub fn is_valid_username(username: &str) -> bool {
    let re = Regex::new(USERNAME_REGEX).unwrap();
    re.is_match(username)
}
