package domain

// UserRepository is a domain port (abstract type).
type UserRepository interface {
	FindByID(id string) (*User, error)
	Save(user *User) error
}

// User is the core domain entity.
type User struct {
	ID   string
	Name string
}
