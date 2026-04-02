package domain

type User struct {
	ID    string
	Name  string
	Email string
}

func (u *User) ChangeName(name string) {
	u.Name = name
}

type UserRepository interface {
	Save(user *User) error
	FindByID(id string) (*User, error)
}
