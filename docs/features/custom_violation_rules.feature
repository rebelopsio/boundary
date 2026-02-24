Feature: Custom Violation Rules (FR-7)
  As an architect
  I want to define project-specific dependency rules in .boundary.toml
  So that boundary enforces my team's architectural constraints beyond the built-in layer rules

  Background:
    Given the project has a .boundary.toml with a custom deny rule:
      | field        | value                                   |
      | name         | no-domain-external                      |
      | from_pattern | .*/domain/.*                            |
      | to_pattern   | .*/external/.*                          |
      | severity     | warning                                 |
      | message      | Domain must not import external packages |

  Scenario: Custom rule matching produces a CustomRule violation
    Given the domain package imports from the external package
    When I run "boundary analyze ."
    Then the output contains a violation of type "custom"
    And the violation identifies the rule "no-domain-external"

  Scenario: Custom violation has the configured severity
    Given the custom rule is configured with severity "warning"
    When I run "boundary analyze ."
    Then the violation is reported with severity "warning"

  Scenario: Custom violation message is set from the config
    Given the custom rule has message "Domain must not import external packages"
    When I run "boundary analyze ."
    Then the output includes the text "Domain must not import external packages"

  Scenario: Rule that doesn't match produces no CustomRule violation
    Given a project with no dependency from domain to external
    When I run "boundary analyze ."
    Then the output does not contain a "custom:" violation

  Scenario: check exits 0 for warning-only custom violations at error threshold
    Given the custom rule fires at severity "warning"
    When I run "boundary check . --fail-on error"
    Then the exit code is 0
