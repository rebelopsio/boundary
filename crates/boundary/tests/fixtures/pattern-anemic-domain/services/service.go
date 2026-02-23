package services

import (
	"github.com/example/pattern-anemic/domain"
)

// OrderService holds business logic, confirming domain is just a data container.
type OrderService struct {
	order domain.Order
}
