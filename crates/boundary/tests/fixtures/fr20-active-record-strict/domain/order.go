package domain

import "github.com/example/fr20/infrastructure/db"

// Order is an Active Record entity: it knows how to persist itself.
type Order struct {
	ID    string
	Total float64
	conn  db.Connection
}

func (o *Order) Save() error {
	return o.conn.Exec("INSERT INTO orders ...")
}

func (o *Order) Load(id string) error {
	return o.conn.Exec("SELECT * FROM orders WHERE id = ?", id)
}
