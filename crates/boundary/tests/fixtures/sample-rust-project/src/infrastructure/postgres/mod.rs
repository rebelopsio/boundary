use crate::domain::user::{User, UserRepository};

pub struct PostgresUserRepository {
    connection_string: String,
}

impl PostgresUserRepository {
    pub fn new(connection_string: String) -> Self {
        Self { connection_string }
    }
}

impl UserRepository for PostgresUserRepository {
    fn save(&self, user: &User) -> Result<(), Error> {
        Ok(())
    }

    fn find_by_id(&self, id: &str) -> Result<User, Error> {
        Ok(User {
            id: id.to_string(),
            name: "test".to_string(),
            email: "test@example.com".to_string(),
        })
    }

    fn delete(&self, id: &str) -> Result<(), Error> {
        Ok(())
    }
}
