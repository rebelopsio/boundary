Feature: Go Adapter Detection

  As an engineer analyzing a Go codebase with Hexagonal Architecture
  I want boundary to correctly identify infrastructure adapters
  So that I can trust the architectural scores to reflect the real structure of my codebase

  Scenario: Application-layer orchestrators are not classified as infrastructure adapters
    Given a Go module where "application/handler.go" contains a UserHandler struct
    When I run "boundary analyze ."
    Then UserHandler is not reported as an infrastructure adapter

  Scenario: An exported struct in the infrastructure layer is classified as an infrastructure component
    Given a Go module where "infrastructure/webhook.go" contains an exported WebhookHandler struct
    When I run "boundary analyze ."
    Then WebhookHandler is classified as an infrastructure component in the Infrastructure layer

  Scenario: An unexported struct in the infrastructure layer is treated as a real component
    Given a Go module where "infrastructure/mongo_repo.go" contains an unexported mongoUserRepository struct
    When I run "boundary analyze ."
    Then mongoUserRepository is classified as a Repository component in the Infrastructure layer

  Scenario: Unexported infrastructure structs are counted toward interface coverage
    Given a Go module where "infrastructure/mongo_repo.go" contains an unexported mongoUserRepository struct
    And "domain/ports.go" defines a UserRepository port interface
    When I run "boundary analyze ."
    Then the Interface Coverage score is greater than zero

  Scenario: An unexported adapter that implements a domain port does not trigger a violation
    Given a Go module where "infrastructure/mongo_repo.go" contains an unexported struct that implements the UserRepository port
    And "domain/ports.go" defines a UserRepository port interface
    When I run "boundary analyze ."
    Then boundary reports no violations for the mongoUserRepository adapter
