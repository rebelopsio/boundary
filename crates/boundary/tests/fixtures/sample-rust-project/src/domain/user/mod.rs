use crate::infrastructure::postgres::PostgresUserRepository;

pub trait UserRepository {
    fn save(&self, user: &User) -> Result<(), Error>;
    fn find_by_id(&self, id: &str) -> Result<User, Error>;
    fn delete(&self, id: &str) -> Result<(), Error>;
}

pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
}
