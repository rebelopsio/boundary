package user

import (
	_ "github.com/example/app/internal/infrastructure/postgres"
)

type User struct {
	ID    string
	Name  string
	Email string
}
