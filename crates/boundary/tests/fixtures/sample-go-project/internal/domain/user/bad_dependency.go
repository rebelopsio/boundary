package user

import (
	"github.com/example/app/internal/infrastructure/postgres"
)

// This is an intentional violation: domain depends on infrastructure
func BadFunction() {
	_ = postgres.NewPostgresUserRepository("bad")
}
