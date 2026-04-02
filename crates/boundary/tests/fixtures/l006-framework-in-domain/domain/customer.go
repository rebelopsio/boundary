package domain

import "time"

type Customer struct {
	ID        string
	Email     string
	CreatedAt time.Time
}

func (c *Customer) Validate() error {
	return nil
}
