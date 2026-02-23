package app

import "github.com/example/fr18-custom-layers/services/auth/core"

// AuthService is an application service.
type AuthService struct{}

func (s *AuthService) GetUser(id string) (*core.User, error) {
	return nil, nil
}
