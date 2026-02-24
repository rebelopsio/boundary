package infrastructure

// Registry holds registered service names.
type Registry struct {
	Services []string
}

// Register adds a service name to the global registry.
func Register(name string) {
}
