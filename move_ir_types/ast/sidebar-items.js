initSidebarItems({"enum":[["Ability","The abilities of a type. Analogous to `move_binary_format::file_format::Ability`."],["BinOp","Enum for binary operators"],["Builtin","Builtin “function”-like operators that often have a signature not expressable in the type system and/or have access to some runtime/storage context"],["Bytecode_",""],["Cmd_","Enum for Move commands"],["CopyableVal_","Bottom of the value hierarchy. These values can be trivially copyable and stored in statedb as a single entry."],["Exp_","Enum for all expressions"],["FunctionBody","The body of a Move function"],["FunctionCall_","Enum for different function calls"],["FunctionVisibility","Public or internal modifier for a procedure"],["LValue_","Enum for Move lvalues"],["ModuleIdent","Either a qualified module name like `addr.m` or `Transaction.m`, which refers to a module in the same transaction."],["ScriptOrModule","A script or a module, used to represent the two types of transactions."],["Statement",""],["StructDefinitionFields","The fields of a Move struct definition"],["Type","The type of a single value"],["UnaryOp","Enum for unary operators"]],"struct":[["BlockLabel",""],["Block_","`{ s }`"],["Constant","A constant declaration in a module or script"],["ConstantName","Newtype for the name of a constant"],["Field_","The field newtype"],["FunctionDependency","An explicit function dependency"],["FunctionName","Newtype for the name of a function"],["FunctionSignature","The signature of a function"],["Function_","A Move function/procedure"],["IfElse","Struct defining an if statement"],["ImportDefinition","A dependency/import declaration"],["Loop","Struct defining a loop statement"],["ModuleDefinition","A Move module"],["ModuleDependency","Explicitly given dependency"],["ModuleName","Newtype for a name of a module"],["NopLabel",""],["Program","A set of Move modules and a Move transaction script"],["QualifiedModuleIdent","Newtype of the address + the module name `addr.m`"],["QualifiedStructIdent","Identifier for a struct definition. Tells us where to look in the storage layer to find the code associated with the interface"],["Script","The Move transaction script to be executed"],["StructDefinition_","A Move struct"],["StructDependency","An explicit struct dependency"],["StructName","Newtype for the name of a struct"],["TypeVar_","New type that represents a type variable. Used to declare type formals & reference them."],["Var_","Newtype for a variable/local"],["While","Struct defining a while statement"]],"type":[["Block","The type of a Block coupled with source location information."],["Bytecode",""],["BytecodeBlock",""],["BytecodeBlocks",""],["Cmd","The type of a command with its location"],["CopyableVal","The type of a value and its location"],["Exp","The type for a `Exp_` and its location"],["ExpFields","The type for fields and their bound expressions"],["Field","A field coupled with source location information"],["Fields","A field map"],["Function","The type of a Function coupled with its source location information."],["FunctionCall","The type for a function call and its location"],["LValue",""],["StructDefinition","The type of a StructDefinition along with its source location information"],["StructTypeParameter","A struct type parameter with its constraints and whether it’s declared as phantom."],["TypeVar","The type of a type variable with a location."],["Var","The type of a variable with a location"]]});