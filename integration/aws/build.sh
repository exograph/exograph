mkdir -p aws-app
id=$(docker create clay-builder-aws:latest)
docker cp $id:/usr/src/app/bootstrap aws-app/
docker rm -v $id