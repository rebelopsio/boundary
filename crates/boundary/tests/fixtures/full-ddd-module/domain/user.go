package domain

type User struct {
	ID    string
	Name  string
	Email string
}

type UserRepository interface {
	Save(user *User) error
	FindByID(id string) (*User, error)
}
