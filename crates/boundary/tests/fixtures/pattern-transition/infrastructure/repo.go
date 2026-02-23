package infrastructure

import (
	"github.com/example/pattern-transition/domain"
)

// OrderRepo is a concrete adapter (no port interface in domain).
type OrderRepo struct {
	order domain.Order
}

// CustomerRepo is a concrete adapter.
type CustomerRepo struct {
	customer domain.Customer
}
