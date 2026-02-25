package infrastructure

import "github.com/example/fr-go-adapters/domain"

type stripePaymentProcessor struct{ apiKey string }

func NewStripePaymentProcessor(apiKey string) domain.PaymentProcessor {
	return &stripePaymentProcessor{apiKey: apiKey}
}
