Feature: Zone of Pain / Zone of Uselessness Detection (FR-26 extension)
  Packages that are far from the main sequence are flagged informally.

  Background:
    # rcm-zone-of-pain: common A=0.0, I=0.0, D=1.0 → Zone of Pain
    # rcm-zone-of-uselessness: abstractions A=1.0, I=1.0, D=1.0 → Zone of Uselessness

  @contract
  Scenario: Zone of Pain package has zone field set to "pain" in JSON output
    Given the fixture "rcm-zone-of-pain"
    When I run "boundary analyze --format json"
    Then the "common" package_metrics entry has zone = "pain"

  @contract
  Scenario: Zone of Uselessness package has zone field set to "uselessness" in JSON output
    Given the fixture "rcm-zone-of-uselessness"
    When I run "boundary analyze --format json"
    Then the "abstractions" package_metrics entry has zone = "uselessness"

  Scenario: Main-sequence package has no zone field in JSON output
    Given the fixture "rcm-ddd-project"
    When I run "boundary analyze --format json"
    Then the "infrastructure" package_metrics entry has no zone field

  Scenario: Text output mentions Zone of Pain when a package is in it
    Given the fixture "rcm-zone-of-pain"
    When I run "boundary analyze" (text output)
    Then the output contains "Zone of Pain"

  Scenario: Text output mentions Zone of Uselessness when a package is in it
    Given the fixture "rcm-zone-of-uselessness"
    When I run "boundary analyze" (text output)
    Then the output contains "Zone of Uselessness"

  Scenario: Text output does not mention zones when no package is in one
    Given the fixture "rcm-ddd-project"
    When I run "boundary analyze" (text output)
    Then the output does not contain "Zone of Pain"
    And the output does not contain "Zone of Uselessness"
