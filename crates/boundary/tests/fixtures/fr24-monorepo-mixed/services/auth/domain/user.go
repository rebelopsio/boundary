package domain

// User is a domain entity.
type User struct {
	ID    string
	Email string
}

// UserRepository defines the port for user persistence.
type UserRepository interface {
	FindByID(id string) (*User, error)
}
