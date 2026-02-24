package domain

// User is the auth domain entity.
type User struct {
	ID    string
	Email string
}

// UserRepository is the auth domain port.
type UserRepository interface {
	FindByID(id string) (*User, error)
}
