package user

// User is a domain entity
type User struct {
	ID    string
	Name  string
	Email string
}

// UserRepository defines the port for user persistence
type UserRepository interface {
	Save(user *User) error
	FindByID(id string) (*User, error)
	Delete(id string) error
}
