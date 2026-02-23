package server

import "github.com/example/fr18-custom-layers/services/auth/core"

// HTTPAdapter is an infrastructure adapter implementing core.UserRepository.
type HTTPAdapter struct{}

func (h *HTTPAdapter) FindByID(id string) (*core.User, error) {
	return nil, nil
}
