# Smart Runner (TODO find a better name)

A convenient way to search through a bunch of command line tools and their gazillion of options.


## How to use

The runner will print the selected command on stdout
(the UI being printed on stderr for the following to work).


### With fish

Bind the command to a key (Control+s) and get the selected command in the shell.
Add the following to your `.config/fish/config.fish` (adjusting <PATH_TO_SMART_RUNNER>):
```
function smart-runner -d "Find a command using tags"
  <PATH_TO_SMART_RUNNER>/smart-runner | read -l cmd
  commandline $cmd
end

function fish_user_key_bindings
    bind \cs smart-runner
end
```


## TODO

* Nix package with fish config