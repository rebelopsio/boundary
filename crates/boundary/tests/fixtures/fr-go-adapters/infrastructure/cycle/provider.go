package infrastructure

import "github.com/example/fr-go-adapters/domain"

// Exported, no adapter suffix — classified as Adapter only via constructor return type.
type CycleInfrastructureProvider struct{ client interface{} }

func NewCycleInfrastructureProvider() domain.InfrastructureProvider {
	return &CycleInfrastructureProvider{}
}
