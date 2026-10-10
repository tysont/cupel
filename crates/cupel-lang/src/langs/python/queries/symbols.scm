; Python symbols: functions, methods (decorated or not), and lambdas bound to names.

(class_definition
  body: (block
    [
      (function_definition
        name: (identifier) @symbol.name
        body: (block) @symbol.body) @symbol.method
      (decorated_definition
        definition: (function_definition
          name: (identifier) @symbol.name
          body: (block) @symbol.body) @symbol.method)
    ]))

(function_definition
  name: (identifier) @symbol.name
  body: (block) @symbol.body) @symbol.function

(assignment
  left: (identifier) @symbol.name
  right: (lambda body: (_) @symbol.body)) @symbol.closure

(class_definition
  name: (identifier) @scope.name) @scope
