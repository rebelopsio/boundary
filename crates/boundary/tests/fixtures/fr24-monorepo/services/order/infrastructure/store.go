package infrastructure

import (
	"github.com/example/fr24/services/order/domain"
	"github.com/example/fr24/shared/logger"
)

// OrderStore implements domain.OrderRepository.
type OrderStore struct {
	log logger.Logger
}

func (s *OrderStore) FindByID(id string) (*domain.Order, error) {
	return nil, nil
}
