package domain

// UserRepository is a port (abstract).
type UserRepository interface {
	Save(user *User) error
	FindByID(id string) (*User, error)
}

// User is a domain entity (concrete).
type User struct {
	ID    string
	Name  string
	Email string
}
