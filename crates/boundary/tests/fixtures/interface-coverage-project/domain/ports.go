package domain

// UserRepository is port #1 (abstract).
type UserRepository interface {
	FindByID(id string) (*User, error)
	Save(user *User) error
}

// OrderRepository is port #2 (abstract).
type OrderRepository interface {
	FindByID(id string) (*Order, error)
	Save(order *Order) error
}

// User is a domain entity.
type User struct {
	ID   string
	Name string
}

// Order is a domain entity.
type Order struct {
	ID     string
	UserID string
}
