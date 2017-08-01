#!/bin/bash

echo '[0,0,"0:function:the_answer",[]]' | msgpack-cli encode
echo '[0,1,"0:function:delay",[1, "Hi"]]' | msgpack-cli encode
echo '[2,"0:function:shutdown",[]]' | msgpack-cli encode
