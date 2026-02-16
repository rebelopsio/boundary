package postgres

import (
	"fmt"
	"github.com/example/app/internal/domain/user"
)

// PostgresUserRepository implements the UserRepository port
type PostgresUserRepository struct {
	connectionString string
}

func NewPostgresUserRepository(connStr string) *PostgresUserRepository {
	return &PostgresUserRepository{connectionString: connStr}
}

func (r *PostgresUserRepository) Save(u *user.User) error {
	fmt.Println("saving user", u.ID)
	return nil
}

func (r *PostgresUserRepository) FindByID(id string) (*user.User, error) {
	return &user.User{ID: id, Name: "test"}, nil
}

func (r *PostgresUserRepository) Delete(id string) error {
	return nil
}
