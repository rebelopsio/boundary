package stripe

import "github.com/example/pa003/domain/ports"

// StripeProcessor implements payment processing via Stripe.
type StripeProcessor struct {
	secretKey string
}

// NewStripeProcessor returns a port interface — this is correct.
func NewStripeProcessor(secretKey string) ports.PaymentProcessor {
	return &StripeProcessor{secretKey: secretKey}
}

func (p *StripeProcessor) Charge(amount int, currency string) error {
	return nil
}
