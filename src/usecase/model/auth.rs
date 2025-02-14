use crate::{domain::model::rules::RuleType, presentation::context::validate::Validator};

pub struct SignUpData {
    pub user_name: String,
    pub password: String,
    pub token: String,
}

impl Validator for SignUpData {
    fn validate(&self) -> anyhow::Result<()> {
        let rules = vec![
            (&self.user_name, RuleType::UserName),
            (&self.password, RuleType::Password),
        ];
        for (field, rule) in rules {
            rule.validate(field)?;
        }
        Ok(())
    }
}

pub struct LoginData {
    pub email: String,
    pub password: String,
}

impl Validator for LoginData {
    fn validate(&self) -> anyhow::Result<()> {
        let rules = vec![(&self.password, RuleType::Password)];
        for (field, rule) in rules {
            rule.validate(field)?;
        }
        Ok(())
    }
}

pub struct ResetPasswordData {
    pub password: String,
    pub token: String,
}

impl Validator for ResetPasswordData {
    fn validate(&self) -> anyhow::Result<()> {
        let rules = vec![(&self.password, RuleType::Password)];
        for (field, rule) in rules {
            rule.validate(field)?;
        }
        Ok(())
    }
}
