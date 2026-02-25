package domain

// UserRepository is a domain port (abstract type).
type UserRepository interface {
	FindByID(id string) (*User, error)
	Save(user *User) error
}

// PaymentProcessor is a domain port for payment processing.
type PaymentProcessor interface {
	Charge(amount float64, currency string) error
}

// InfrastructureProvider is a domain port for infrastructure provisioning.
type InfrastructureProvider interface {
	Provision(resource string) error
}

// User is the core domain entity.
type User struct {
	ID   string
	Name string
}
