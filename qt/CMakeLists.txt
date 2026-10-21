cmake_minimum_required(VERSION 3.16)
project(paa-viewer-qt LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

find_package(Qt6 REQUIRED COMPONENTS Core Gui Widgets)

add_executable(paa-viewer-qt
    src/main.cpp
    src/mainwindow.cpp
    src/mainwindow.h
    src/paa_ffi.h
)

target_include_directories(paa-viewer-qt PRIVATE src)

get_filename_component(PROJECT_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/.." ABSOLUTE)
set(RUST_LIB_DIR "${PROJECT_ROOT}/target/release")

target_link_libraries(paa-viewer-qt PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    "${RUST_LIB_DIR}/libpaa_viewer.so"
)

set_target_properties(paa-viewer-qt PROPERTIES
    BUILD_RPATH "${RUST_LIB_DIR};$ORIGIN;$ORIGIN/../lib"
    INSTALL_RPATH "$ORIGIN;$ORIGIN/../lib"
)
