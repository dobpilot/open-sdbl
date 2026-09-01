FROM maven:3.9.9-eclipse-temurin-24 AS build
WORKDIR /source
COPY pom.xml ./
COPY src ./src
RUN mvn -q package

FROM scratch AS artifact
COPY --from=build /source/target/plugin /plugin/open-sdbl
