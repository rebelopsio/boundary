package ports

// PaymentProcessor defines the contract for processing payments.
type PaymentProcessor interface {
	Charge(amount int, currency string) error
}
