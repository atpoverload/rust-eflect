load("@rules_python//python:defs.bzl", "py_binary", "py_library")

py_library(
  name = "processing",
  srcs = ["processing.py"],
  visibility = ["//visibility:public"],
  deps = [":processing"]
)

py_binary(
  name = "py_client",
  srcs = ["client.py"],
  visibility = ["//visibility:public"],
  deps = [":processing"]
)


py_binary(
  name = "py_harness",
  srcs = ["harness.py"],
  visibility = ["//visibility:public"],
  deps = [":processing"]
)
