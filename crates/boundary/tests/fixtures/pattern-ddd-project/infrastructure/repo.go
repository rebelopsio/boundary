package infrastructure

import (
	"github.com/example/pattern-ddd/domain"
)

// UserRepo is a concrete infrastructure adapter.
type UserRepo struct {
	domain domain.UserRepository
}
