Feature: Init Function Dependency Detection (FR-21)
  Go init() functions that call across layer boundaries create hidden coupling.
  Boundary detects these as InitFunctionCoupling violations (warning severity by
  default) and can be disabled via detect_init_functions = false.

  Background:
    # fixture: fr21-init-coupling
    #   domain/setup.go     — init() calls infrastructure.Register() (layer violation)
    #   infrastructure/registry.go — Register() function

  @contract
  Scenario: init() cross-layer call produces an InitFunctionCoupling violation
    Given a fixture where a domain init() calls into the infrastructure layer
    When I run "boundary analyze --format json"
    Then the violations list contains a violation with kind InitFunctionCoupling

  @contract
  Scenario: InitFunctionCoupling violation has warning severity by default
    Given a fixture with an init() cross-layer call
    When I run "boundary analyze --format json"
    Then the InitFunctionCoupling violation has severity "warning"

  Scenario: detect_init_functions = false suppresses init violations
    Given a fixture where a domain init() calls into the infrastructure layer
    And .boundary.toml sets detect_init_functions = false
    When I run "boundary analyze --format json"
    Then no InitFunctionCoupling violation is reported

  Scenario: Text output describes the init coupling location
    Given a fixture with an init() cross-layer call
    When I run "boundary analyze" (text output)
    Then the output contains "init"
