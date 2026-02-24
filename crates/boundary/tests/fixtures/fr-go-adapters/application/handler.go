package application

// UserHandler is an application-layer orchestrator.
// It coordinates use cases via domain ports — it is NOT a hexagonal adapter.
type UserHandler struct {
	ID   string
	Name string
}
