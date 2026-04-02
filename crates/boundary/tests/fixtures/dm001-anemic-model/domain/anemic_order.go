package domain

import "time"

type Order struct {
	ID         string
	CustomerID string
	Items      []string
	Total      float64
	Status     string
	CreatedAt  time.Time
	UpdatedAt  time.Time
}

func (o *Order) GetID() string      { return o.ID }
func (o *Order) GetTotal() float64  { return o.Total }
func (o *Order) GetStatus() string  { return o.Status }
