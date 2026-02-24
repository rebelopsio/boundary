package domain

import "github.com/example/fr22/services/legacy/infrastructure"

// Order is a legacy Active Record entity.
type Order struct {
	ID    string
	Total float64
}

func (o *Order) Save() error {
	return infrastructure.Exec("INSERT INTO orders ...")
}
