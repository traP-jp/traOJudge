pub mod rules;

pub enum RuleType {
    UserName,
    Password,
    Icon,
    XLink,
    GitHubLink,
    SelfIntroduction,
}

impl RuleType {
    fn get_rule(&self) -> &'static str {
        match self {
            RuleType::UserName => rules::USER_NAME_RULE,
            RuleType::Password => rules::PASSWORD_RULE,
            RuleType::Icon => rules::ICON_RULE,
            RuleType::XLink => rules::X_LINK_RULE,
            RuleType::GitHubLink => rules::GITHUB_LINK_RULE,
            RuleType::SelfIntroduction => rules::SELF_INTRODUCTION_RULE,
        }
    }
    pub fn validate(&self, value: &str) -> anyhow::Result<()> {
        let rule = self.get_rule();
        let re = regex::Regex::new(rule)?;
        if re.is_match(value) {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Invalid value"))
        }
    }
}

pub trait Validator {
    fn validate(&self) -> anyhow::Result<()>;
}
