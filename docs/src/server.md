# Server setup

## Using Docker Compose
The easiest way to run the server is to use the bundled configuration files for Docker Compose on Linux. Docker with automatically build and launch a container with the server executable and the required PostgreSQL database. To use this method, please ensure you have Docker and Docker Compose installed and are in the root folder of the implementation code.

Now, run the following command:
```bash
docker-compose up --build
```

After the build process, the server should now be running locally with port `9999`.

If you want to destroy all data and run everything again, run this command:
```bash
docker-compose down
```

## Manual build process
It's also possible to run the server without Docker. The server binary can be compiled and launched in a similar way to the client binary, except it also depends on PostgreSQL development libraries.

The PostgreSQL database needs to be run and managed separately. Credentials for the database can be set up in the `server/.env` file.

## Production deployment
For a production deployment, it's recommended to use the Docker setup, along with a reverse proxy (like nginx) that provides TLS support with a valid certificate. Using TLS in production is mandatory.
