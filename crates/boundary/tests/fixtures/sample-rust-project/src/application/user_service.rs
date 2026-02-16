use crate::domain::user::{User, UserRepository};

pub struct UserService {
    repo: Box<dyn UserRepository>,
}

impl UserService {
    pub fn new(repo: Box<dyn UserRepository>) -> Self {
        Self { repo }
    }

    pub fn create_user(&self, name: &str, email: &str) -> Result<User, Error> {
        let user = User {
            id: "generated".to_string(),
            name: name.to_string(),
            email: email.to_string(),
        };
        self.repo.save(&user)?;
        Ok(user)
    }
}
